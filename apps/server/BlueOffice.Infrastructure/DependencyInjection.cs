using BlueOffice.Domain;
using BlueOffice.Infrastructure.Data;
using BlueOffice.Infrastructure.Hosts;
using BlueOffice.Infrastructure.Jobs;
using BlueOffice.Infrastructure.Storage;
using Microsoft.EntityFrameworkCore;
using Microsoft.Extensions.Configuration;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.DependencyInjection.Extensions;
using Microsoft.Extensions.Hosting;
using Microsoft.Extensions.Logging;
using Minio;
using StackExchange.Redis;

namespace BlueOffice.Infrastructure;

/// <summary>Service registration for the infrastructure layer.</summary>
public static class DependencyInjection
{
    /// <summary>
    /// Registers database (<c>Database:Provider</c> = Sqlite | Postgres, <c>ConnectionStrings:Default</c>),
    /// object store (<c>Storage</c>), job queue (<c>Jobs</c>, <c>ConnectionStrings:Redis</c>), bo-cli worker (<c>BoCli</c>)
    /// and host clients (<c>Hosts</c>).
    /// </summary>
    public static IServiceCollection AddBlueOfficeInfrastructure(this IServiceCollection services, IConfiguration config)
    {
        services.TryAddSingleton(TimeProvider.System);

        // Database
        var provider = config["Database:Provider"] ?? "Sqlite";
        var cs = config.GetConnectionString("Default") ?? "Data Source=data/blueoffice.db";
        services.AddDbContext<BlueOfficeDbContext>(o =>
        {
            if (provider.Equals("Postgres", StringComparison.OrdinalIgnoreCase))
            {
                o.UseNpgsql(cs);
            }
            else
            {
                o.UseSqlite(cs);
            }
        });

        // Object store
        var storage = new StorageOptions();
        config.GetSection("Storage").Bind(storage);
        services.AddSingleton(storage);
        if (storage.Provider.Equals("Minio", StringComparison.OrdinalIgnoreCase))
        {
            services.AddSingleton<IObjectStore>(_ =>
            {
                var client = new MinioClient()
                    .WithEndpoint(storage.Endpoint)
                    .WithCredentials(storage.AccessKey, storage.SecretKey)
                    .WithSSL(storage.UseSsl)
                    .Build();
                return new MinioObjectStore(client, storage.Bucket);
            });
        }
        else
        {
            services.AddSingleton<IObjectStore>(_ => new FileSystemObjectStore(storage.Root));
        }

        // Job queue + worker pieces
        var jobs = new JobQueueOptions();
        config.GetSection("Jobs").Bind(jobs);
        services.AddSingleton(jobs);
        if (jobs.Provider.Equals("Redis", StringComparison.OrdinalIgnoreCase))
        {
            var redisCs = config.GetConnectionString("Redis") ?? "localhost:6379";
            services.AddSingleton<IConnectionMultiplexer>(_ => ConnectionMultiplexer.Connect(redisCs));
            services.AddSingleton<IRedisCommands, RedisCommands>();
            services.AddSingleton<IJobQueue, RedisJobQueue>();
        }
        else
        {
            services.AddSingleton<IJobQueue, InMemoryJobQueue>();
        }

        services.Configure<BoCliOptions>(config.GetSection("BoCli"));
        services.AddSingleton<IBoCliRunner, BoCliProcessRunner>();
        services.AddSingleton<ConversionJobProcessor>();

        // Hosts (BOHP / WOPI)
        services.Configure<HostsOptions>(o => config.GetSection("Hosts").Bind(o.Items));
        services.AddSingleton<HostRegistry>();
        services.AddSingleton<HostTokenValidator>();
        services.AddHttpClient(HostTokenValidator.HttpClientName, c => c.Timeout = TimeSpan.FromSeconds(10));
        services.AddHttpClient(BohpClient.HttpClientName, c => c.Timeout = TimeSpan.FromSeconds(60));
        services.AddHttpClient(WopiClient.HttpClientName, c => c.Timeout = TimeSpan.FromSeconds(60));
        services.AddHttpClient(HostWebhookDispatcher.HttpClientName, c => c.Timeout = TimeSpan.FromSeconds(10));
        services.AddSingleton<IBohpClient, BohpClient>();
        services.AddSingleton(sp => WopiProofKeys.Load(
            config,
            sp.GetRequiredService<IHostEnvironment>(),
            config.GetSection("Hosts").GetChildren().Any(h => string.Equals(h["Protocol"], "Wopi", StringComparison.OrdinalIgnoreCase)),
            sp.GetRequiredService<ILoggerFactory>().CreateLogger("BlueOffice.Wopi")));
        services.AddSingleton<IWopiClient, WopiClient>();
        services.Configure<WebhookOptions>(config.GetSection("Webhooks"));
        services.AddSingleton<HostWebhookDispatcher>();
        services.AddSingleton<IHostNotifier>(sp => sp.GetRequiredService<HostWebhookDispatcher>());
        services.AddSingleton<IHostedService>(sp => sp.GetRequiredService<HostWebhookDispatcher>());
        return services;
    }
}
