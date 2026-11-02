# syntax=docker/dockerfile:1
FROM python:3.12.14-slim
WORKDIR /app
COPY requirements.txt .
RUN --mount=type=secret,id=proxy_ca \
    if [ -f /run/secrets/proxy_ca ]; then export PIP_CERT=/run/secrets/proxy_ca; fi; \
    pip install --no-cache-dir -r requirements.txt \
    && useradd --uid 10001 --no-create-home --shell /usr/sbin/nologin relay
COPY --chmod=0555 dhrelay ./dhrelay
USER 10001:10001
EXPOSE 8765
HEALTHCHECK --interval=30s --timeout=3s CMD python -c "import urllib.request; urllib.request.build_opener(urllib.request.ProxyHandler({})).open('http://127.0.0.1:8765/healthz', timeout=2)" || exit 1
CMD ["python", "-m", "dhrelay", "--host", "0.0.0.0"]
