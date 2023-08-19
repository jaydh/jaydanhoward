FROM debian:bullseye-slim AS runtime 

RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates curl chromium uuid-runtime

RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash - && apt-get install -y nodejs
ARG CACHEBUST=1
RUN npm install -g npm
RUN npm install -g lighthouse

RUN apt-get autoremove -y \
    && apt-get clean -y \
    && apt-get purge --auto-remove -y gnupg \
    && rm -rf /var/lib/apt/lists/* 



RUN groupadd -r chrome && useradd -r -g chrome -G audio,video chrome \
  && mkdir -p /home/chrome && chown -R chrome:chrome /home/chrome

USER chrome
WORKDIR /home/chrome
ENV LEPTOS_SITE_ADDR="https://jaydanhoward-29npi.ondigitalocean.app"

COPY entrypoint.sh entrypoint.sh
ENTRYPOINT ["./entrypoint.sh"]
