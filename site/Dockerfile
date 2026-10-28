# Built from the repository root, since it needs docs/plugins.md: docker build -f site/Dockerfile .
FROM python:3.13-alpine AS build
WORKDIR /repo
COPY docs/plugins.md docs/plugins.md
COPY site site
RUN sh site/build.sh /site

FROM nginx:1.27-alpine
COPY --from=build /site /usr/share/nginx/html
