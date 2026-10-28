FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --locked --release -p esp-network-backend

FROM node:22-bookworm-slim
WORKDIR /app
COPY --from=build /src/target/release/esp-network-backend /usr/local/bin/esp-network-backend
COPY tools/network-relay.mjs tools/network-helper.mjs ./tools/
USER node
ENV RELAY_HOST=0.0.0.0 RELAY_PORT=8788 RELAY_BACKEND=/usr/local/bin/esp-network-backend
EXPOSE 8788
HEALTHCHECK --interval=30s --timeout=3s CMD node -e "fetch('http://127.0.0.1:8788/health').then(r=>{if(!r.ok)process.exit(1)}).catch(()=>process.exit(1))"
CMD ["node", "tools/network-relay.mjs"]
