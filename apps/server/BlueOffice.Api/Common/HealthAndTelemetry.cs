using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using Microsoft.Extensions.Diagnostics.HealthChecks;
using OpenTelemetry.Metrics;
using OpenTelemetry.Resources;
using OpenTelemetry.Trace;
using StackExchange.Redis;

namespace BlueOffice.Api.Common;

/// <summary>Object store reachability.</summary>
public sealed class ObjectStoreHealthCheck(IObjectStore store) : IHealthCheck
{
    /// <inheritdoc />
    public async Task<HealthCheckResult> CheckHealthAsync(HealthCheckContext context, CancellationToken cancellationToken = default)
    {
        try
        {
            await store.ExistsAsync("health/probe", cancellationToken);
            return HealthCheckResult.Healthy();
        }
        catch (Exception e) when (e is not OperationCanceledException)
        {
            return HealthCheckResult.Unhealthy("Object store unreachable.", e);
        }
    }
}

/// <summary>Redis reachability (only registered with the Redis job queue).</summary>
public sealed class RedisHealthCheck(IConnectionMultiplexer redis) : IHealthCheck
{
    /// <inheritdoc />
    public Task<HealthCheckResult> CheckHealthAsync(HealthCheckContext context, CancellationToken cancellationToken = default) =>
        Task.FromResult(redis.IsConnected ? HealthCheckResult.Healthy() : HealthCheckResult.Unhealthy("Redis disconnected."));
}

/// <summary>Health checks and OpenTelemetry registration.</summary>
public static class HealthAndTelemetry
{
    /// <summary>Tag of readiness checks.</summary>
    public const string ReadyTag = "ready";

    /// <summary>Registers <c>/health</c> (liveness) and <c>/health/ready</c> checks.</summary>
    public static IServiceCollection AddBlueOfficeHealth(this IServiceCollection services, IConfiguration config)
    {
        var hc = services.AddHealthChecks()
            .AddDbContextCheck<BlueOfficeDbContext>("database", tags: [ReadyTag])
            .AddCheck<ObjectStoreHealthCheck>("object-store", tags: [ReadyTag]);
        if (string.Equals(config["Jobs:Provider"], "Redis", StringComparison.OrdinalIgnoreCase))
        {
            hc.AddCheck<RedisHealthCheck>("redis", tags: [ReadyTag]);
        }

        return services;
    }

    /// <summary>
    /// Traces + metrics (ASP.NET Core, HttpClient, sources "BlueOffice.*", meter "BlueOffice").
    /// OTLP export is enabled when <c>Otel:Endpoint</c> is set (e.g. http://otel-collector:4317).
    /// </summary>
    public static IServiceCollection AddBlueOfficeTelemetry(this IServiceCollection services, IConfiguration config)
    {
        var endpoint = config["Otel:Endpoint"];
        var serviceName = config["Otel:ServiceName"] ?? "blueoffice-api";
        services.AddOpenTelemetry()
            .ConfigureResource(r => r.AddService(serviceName))
            .WithTracing(t =>
            {
                t.AddAspNetCoreInstrumentation().AddHttpClientInstrumentation().AddSource("BlueOffice.*");
                if (!string.IsNullOrEmpty(endpoint))
                {
                    t.AddOtlpExporter(o => o.Endpoint = new Uri(endpoint));
                }
            })
            .WithMetrics(m =>
            {
                m.AddAspNetCoreInstrumentation().AddHttpClientInstrumentation().AddMeter("BlueOffice");
                if (!string.IsNullOrEmpty(endpoint))
                {
                    m.AddOtlpExporter(o => o.Endpoint = new Uri(endpoint));
                }
            });
        return services;
    }
}
