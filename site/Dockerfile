# Built from the repository root, since the site renders docs/ and reads Cargo.toml:
# docker build -f site/Dockerfile .
FROM node:24-alpine AS build
RUN corepack enable
WORKDIR /repo/site
COPY site/package.json site/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY Cargo.toml /repo/Cargo.toml
COPY docs /repo/docs
COPY site /repo/site
RUN pnpm build

FROM nginx:1.27-alpine
COPY --from=build /repo/site/dist /usr/share/nginx/html
