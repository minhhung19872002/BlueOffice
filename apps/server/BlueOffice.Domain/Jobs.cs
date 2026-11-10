namespace BlueOffice.Domain;

/// <summary>Kinds of conversion work executed by the bo-cli worker.</summary>
public enum JobKind
{
    /// <summary><c>bo-cli convert --to pdf</c>.</summary>
    Convert,

    /// <summary><c>bo-cli render --page 1 --scale s</c> → PNG.</summary>
    Thumbnail,

    /// <summary>Quick view for read-only users: PDF of the whole document, or one PNG per page (<c>format=png</c>).</summary>
    Preview,

    /// <summary>Plain text from the display list (<c>bo-cli dump</c>).</summary>
    ExtractText,

    /// <summary>Mail merge of a .docx template with a data file (<c>bo-cli merge</c>, REST API).</summary>
    Merge,

    /// <summary>Template filling from a JSON list of edits (<c>bo-cli edit</c>, REST API).</summary>
    Edit,

    /// <summary>Format, variant, encryption and macros of a file (<c>bo-cli inspect --json</c>, REST API).</summary>
    Inspect,
}

/// <summary>Job lifecycle.</summary>
public enum JobStatus
{
    /// <summary>Waiting in the queue.</summary>
    Queued,

    /// <summary>Picked up by a worker.</summary>
    Running,

    /// <summary>Output available.</summary>
    Succeeded,

    /// <summary>Failed; see <see cref="ConversionJob.Error"/>.</summary>
    Failed,
}

/// <summary>A conversion job. Input and output bytes live in the object store under <c>jobs/&lt;id&gt;/</c>.</summary>
public sealed class ConversionJob
{
    /// <summary>Id (32 hex chars).</summary>
    public string Id { get; set; } = Guid.NewGuid().ToString("N");

    /// <summary>Kind.</summary>
    public JobKind Kind { get; set; }

    /// <summary>Status.</summary>
    public JobStatus Status { get; set; }

    /// <summary>Original file name (used only for the extension and the output name).</summary>
    public string FileName { get; set; } = "";

    /// <summary>Requested target format: <see cref="JobKind.Convert"/> (e.g. "pdf", "docx"), <see cref="JobKind.Preview"/> ("pdf" or "png").</summary>
    public string? TargetFormat { get; set; }

    /// <summary>Page (1-based) for thumbnails.</summary>
    public int Page { get; set; } = 1;

    /// <summary>Scale (px per pt) for thumbnails.</summary>
    public double Scale { get; set; } = 0.5;

    /// <summary>Principal that created the job (only it may read the result).</summary>
    public string Owner { get; set; } = "";

    /// <summary>Object key of the input.</summary>
    public string InputKey => $"jobs/{Id}/input";

    /// <summary>Object key of the output.</summary>
    public string OutputKey => $"jobs/{Id}/output";

    /// <summary>Object key of the second input: merge data (<see cref="JobKind.Merge"/>) or the edits JSON (<see cref="JobKind.Edit"/>).</summary>
    public string DataKey => $"jobs/{Id}/data";

    /// <summary>Extension of the merge data file (".csv", ".json", ".xlsx", ".vcf").</summary>
    public string? DataExtension { get; set; }

    /// <summary>
    /// Extra options by kind — convert: <c>pdfa</c> (1b|2b|3b), <c>tagged</c>, <c>bookmarks</c>; merge: <c>each</c> (pdf|docx),
    /// <c>pattern</c>; edit: <c>strict</c>. Values are validated by the API before the job is queued.
    /// </summary>
    public Dictionary<string, string> Options { get; set; } = [];

    /// <summary>Prefix of the status and result URLs ("" for the host services, "/api/v1" for the REST API).</summary>
    public string UrlPrefix { get; set; } = "";

    /// <summary>Edits that changed something (<see cref="JobKind.Edit"/>).</summary>
    public int? EditsApplied { get; set; }

    /// <summary>Edits that matched nothing (<see cref="JobKind.Edit"/>).</summary>
    public int? EditsUnmatched { get; set; }

    /// <summary>Object key of page <paramref name="page"/> (1-based) of a PNG preview.</summary>
    public string PageKey(int page) => $"jobs/{Id}/pages/{page}";

    /// <summary>Number of page images stored by a PNG preview (null for other jobs).</summary>
    public int? OutputPages { get; set; }

    /// <summary>MIME type of the output.</summary>
    public string? OutputContentType { get; set; }

    /// <summary>File name of the output.</summary>
    public string? OutputFileName { get; set; }

    /// <summary>Error message when failed.</summary>
    public string? Error { get; set; }

    /// <summary>Creation time (UTC).</summary>
    public DateTime CreatedAt { get; set; }

    /// <summary>Completion time (UTC).</summary>
    public DateTime? CompletedAt { get; set; }
}

/// <summary>Queue + status store for conversion jobs (Redis in production, in-memory for tests).</summary>
public interface IJobQueue
{
    /// <summary>Saves the job and appends it to the queue.</summary>
    Task EnqueueAsync(ConversionJob job, CancellationToken ct);

    /// <summary>Takes the next job, waiting until one is available or <paramref name="ct"/> is cancelled.</summary>
    Task<ConversionJob> DequeueAsync(CancellationToken ct);

    /// <summary>Persists a status change.</summary>
    Task UpdateAsync(ConversionJob job, CancellationToken ct);

    /// <summary>Loads a job by id.</summary>
    Task<ConversionJob?> GetAsync(string id, CancellationToken ct);
}

/// <summary>Result of running bo-cli.</summary>
public sealed record ProcessResult(int ExitCode, string StdOut, string StdErr);

/// <summary>Runs the bo-cli executable (fakeable in tests).</summary>
public interface IBoCliRunner
{
    /// <summary>Runs bo-cli with <paramref name="args"/> in <paramref name="workingDirectory"/>.</summary>
    Task<ProcessResult> RunAsync(IReadOnlyList<string> args, string workingDirectory, CancellationToken ct);
}
