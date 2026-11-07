# BlueOffice server image: ASP.NET Core API/worker + headless bo-cli (Rust).
# Build from the repository root:  docker build -f deploy/Dockerfile -t blueoffice/server:0.6.0 .

# ---- bo-cli (Rust, release) ------------------------------------------------
FROM rust:1.85-bookworm AS bocli
WORKDIR /src
COPY Cargo.toml Cargo.lock deny.toml ./
COPY crates crates
COPY assets assets
# The workspace lists the desktop shell as a member; only its manifest is needed to resolve the workspace.
COPY apps/desktop/src-tauri apps/desktop/src-tauri
RUN cargo build --release --locked -p bo-cli

# ---- .NET publish ------------------------------------------------------------
FROM mcr.microsoft.com/dotnet/sdk:8.0 AS dotnet
WORKDIR /src
COPY apps/server apps/server
RUN dotnet publish apps/server/BlueOffice.Api/BlueOffice.Api.csproj -c Release -o /out /p:UseAppHost=false

# ---- runtime -----------------------------------------------------------------
FROM mcr.microsoft.com/dotnet/aspnet:8.0
RUN apt-get update && apt-get install -y --no-install-recommends curl tzdata && rm -rf /var/lib/apt/lists/* \
    && groupadd --system blueoffice && useradd --system --gid blueoffice --home /app blueoffice \
    && mkdir -p /data && chown blueoffice:blueoffice /data
WORKDIR /app
COPY --from=dotnet /out ./
COPY --from=bocli /src/target/release/bo-cli /usr/local/bin/bo-cli
ENV ASPNETCORE_URLS=http://+:8080 \
    ASPNETCORE_ENVIRONMENT=Production \
    BoCli__ExecutablePath=/usr/local/bin/bo-cli \
    TZ=Asia/Ho_Chi_Minh
USER blueoffice
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --retries=3 CMD curl -fsS http://localhost:8080/health || exit 1
ENTRYPOINT ["dotnet", "BlueOffice.Api.dll"]
