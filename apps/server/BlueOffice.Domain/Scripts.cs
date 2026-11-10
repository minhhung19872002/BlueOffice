namespace BlueOffice.Domain;

/// <summary>
/// A BlueScript run on a schedule for one file of a host (CORE-SCRIPT-002). The code is a copy of one of the user's
/// saved scripts, sent explicitly by the user from the BlueScript pane — scripts embedded in documents are never
/// scheduled or run automatically (ADR-0012).
/// </summary>
public sealed class ScriptSchedule
{
    /// <summary>Id (32 hex chars).</summary>
    public string Id { get; set; } = Guid.NewGuid().ToString("N");

    /// <summary>Host name (<see cref="HostConfig.Name"/>).</summary>
    public string Host { get; set; } = "";

    /// <summary>File id in the host.</summary>
    public string FileId { get; set; } = "";

    /// <summary>File name when the schedule was made (picks the application: doc / sheet / slide).</summary>
    public string FileName { get; set; } = "";

    /// <summary>Host user id of the owner.</summary>
    public string UserId { get; set; } = "";

    /// <summary>Display name of the owner.</summary>
    public string UserName { get; set; } = "";

    /// <summary>Script name.</summary>
    public string Name { get; set; } = "";

    /// <summary>Script source (JavaScript, BlueScript API).</summary>
    public string Code { get; set; } = "";

    /// <summary>Five-field cron expression (UTC), see <see cref="CronExpression"/>.</summary>
    public string Cron { get; set; } = "";

    /// <summary>Whether the worker runs it.</summary>
    public bool Enabled { get; set; } = true;

    /// <summary>Next due time (UTC); null when the expression never fires again.</summary>
    public DateTime? NextRunAt { get; set; }

    /// <summary>Last run (UTC).</summary>
    public DateTime? LastRunAt { get; set; }

    /// <summary><c>succeeded</c>, <c>failed</c> or <c>unavailable</c> (no runner configured).</summary>
    public string? LastStatus { get; set; }

    /// <summary>Error of the last run.</summary>
    public string? LastError { get; set; }

    /// <summary>Creation time (UTC).</summary>
    public DateTime CreatedAt { get; set; }
}

/// <summary>Outcome of one scheduled run.</summary>
/// <param name="Status"><c>succeeded</c>, <c>failed</c> or <c>unavailable</c>.</param>
/// <param name="Error">Error message when not succeeded.</param>
public sealed record ScriptRunResult(string Status, string? Error)
{
    /// <summary>Status of a successful run.</summary>
    public const string Succeeded = "succeeded";

    /// <summary>Status of a failed run.</summary>
    public const string Failed = "failed";

    /// <summary>Status when no server-side runner exists.</summary>
    public const string Unavailable = "unavailable";
}

/// <summary>Runs a scheduled script headless against the host file (opens it, runs the script, saves a version).</summary>
public interface IScriptRunner
{
    /// <summary>Runs <paramref name="schedule"/> once.</summary>
    Task<ScriptRunResult> RunAsync(ScriptSchedule schedule, CancellationToken ct);
}

/// <summary>
/// Five-field cron expression, UTC: <c>minute hour day-of-month month day-of-week</c>. Each field takes <c>*</c>,
/// numbers, ranges <c>a-b</c>, lists <c>a,b</c> and steps <c>*/n</c> / <c>a-b/n</c>; day-of-week is 0–7 (0 and 7 are
/// Sunday). As in classic cron, when both day fields are restricted a day matches either of them.
/// </summary>
public sealed class CronExpression
{
    private readonly bool[] _minutes;
    private readonly bool[] _hours;
    private readonly bool[] _days;
    private readonly bool[] _months;
    private readonly bool[] _weekdays;
    private readonly bool _anyDay;
    private readonly bool _anyWeekday;

    private CronExpression(bool[] minutes, bool[] hours, bool[] days, bool[] months, bool[] weekdays, bool anyDay, bool anyWeekday)
    {
        _minutes = minutes;
        _hours = hours;
        _days = days;
        _months = months;
        _weekdays = weekdays;
        _anyDay = anyDay;
        _anyWeekday = anyWeekday;
    }

    /// <summary>Parses an expression; throws <see cref="FormatException"/> when malformed.</summary>
    public static CronExpression Parse(string expression)
    {
        ArgumentNullException.ThrowIfNull(expression);
        var f = expression.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries);
        if (f.Length != 5)
        {
            throw new FormatException("A schedule has five fields: minute hour day month weekday.");
        }

        var weekdays = Field(f[4], 0, 7);
        weekdays[0] |= weekdays[7];
        return new CronExpression(
            Field(f[0], 0, 59), Field(f[1], 0, 23), Field(f[2], 1, 31), Field(f[3], 1, 12), weekdays[..7], f[2] == "*", f[4] == "*");
    }

    /// <summary>Parses an expression; false when malformed.</summary>
    public static bool TryParse(string? expression, out CronExpression? cron)
    {
        try
        {
            cron = expression is null ? null : Parse(expression);
            return cron is not null;
        }
        catch (FormatException)
        {
            cron = null;
            return false;
        }
    }

    /// <summary>First matching minute strictly after <paramref name="afterUtc"/>; null if none within 5 years.</summary>
    public DateTime? Next(DateTime afterUtc)
    {
        var t = new DateTime(afterUtc.Year, afterUtc.Month, afterUtc.Day, afterUtc.Hour, afterUtc.Minute, 0, DateTimeKind.Utc).AddMinutes(1);
        var day = t.Date;
        var end = day.AddYears(5);
        for (; day < end; day = day.AddDays(1))
        {
            if (!_months[day.Month] || !DayMatches(day))
            {
                continue;
            }

            var startHour = day == t.Date ? t.Hour : 0;
            for (var h = startHour; h < 24; h++)
            {
                if (!_hours[h])
                {
                    continue;
                }

                var startMinute = day == t.Date && h == t.Hour ? t.Minute : 0;
                for (var m = startMinute; m < 60; m++)
                {
                    if (_minutes[m])
                    {
                        return DateTime.SpecifyKind(day.AddHours(h).AddMinutes(m), DateTimeKind.Utc);
                    }
                }
            }
        }

        return null;
    }

    private bool DayMatches(DateTime day)
    {
        var dom = _days[day.Day];
        var dow = _weekdays[(int)day.DayOfWeek];
        if (_anyDay && _anyWeekday)
        {
            return true;
        }

        if (_anyDay)
        {
            return dow;
        }

        return _anyWeekday ? dom : dom || dow;
    }

    private static bool[] Field(string text, int min, int max)
    {
        var set = new bool[max + 1];
        foreach (var part in text.Split(','))
        {
            var step = 1;
            var range = part;
            var slash = part.IndexOf('/', StringComparison.Ordinal);
            if (slash >= 0)
            {
                step = Number(part[(slash + 1)..], 1, max);
                range = part[..slash];
            }

            int lo;
            int hi;
            if (range == "*")
            {
                (lo, hi) = (min, max);
            }
            else if (range.IndexOf('-', StringComparison.Ordinal) is var dash and > 0)
            {
                lo = Number(range[..dash], min, max);
                hi = Number(range[(dash + 1)..], min, max);
                if (hi < lo)
                {
                    throw new FormatException($"Bad range '{range}'.");
                }
            }
            else
            {
                lo = Number(range, min, max);
                hi = slash >= 0 ? max : lo;
            }

            for (var v = lo; v <= hi; v += step)
            {
                set[v] = true;
            }
        }

        return set;
    }

    private static int Number(string s, int min, int max) =>
        int.TryParse(s, System.Globalization.NumberStyles.None, System.Globalization.CultureInfo.InvariantCulture, out var v) && v >= min && v <= max
            ? v
            : throw new FormatException($"'{s}' is not a number from {min} to {max}.");
}
