FROM python:3.12-slim-trixie
COPY gil /usr/local/bin/gil
COPY adapter.py run-worker.py latencies.csv /case/
ENV GIL_BENCHMARK_MODEL=luna
RUN chmod 755 /case/adapter.py && mkdir -p /work/model && chown -R 65534:65534 /work
