# Built from the repository root, since it renders docs/: docker build -f site/Dockerfile .
FROM python:3.13-alpine AS build
WORKDIR /repo
COPY docs docs
COPY site site
RUN python3 site/build.py /site

FROM nginx:1.27-alpine
COPY --from=build /site /usr/share/nginx/html
