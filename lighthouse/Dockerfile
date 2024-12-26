FROM debian:bookworm-slim AS runtime

RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates curl gnupg chromium chromium-sandbox

RUN mkdir -p /etc/apt/keyrings
RUN curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg

ENV NODE_MAJOR=20
RUN echo "deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_$NODE_MAJOR.x nodistro main" | tee /etc/apt/sources.list.d/nodesource.list

RUN apt-get update
RUN apt-get install -y nodejs
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

ENV LEPTOS_SITE_HEALTH_ADDR="https://jaydanhoward.com/health_check"
ENV LEPTOS_SITE_API_ADDR="https://jaydanhoward.com/api/lighthouse"
ENV LEPTOS_SITE_TARGET_ADDR="https://jaydanhoward.com/about"

COPY entrypoint.sh entrypoint.sh
ENTRYPOINT ["./entrypoint.sh"]
