# Web shell image. Build the app first (needs Rust + wasm-bindgen for the engine):
#   pnpm install && pnpm build:wasm && VITE_EMBED_ORIGINS=https://drive.example.vn pnpm --filter @blueoffice/web build
FROM nginx:1.27-alpine
COPY deploy/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY apps/web/dist /usr/share/nginx/html
EXPOSE 8081
