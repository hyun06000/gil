FROM python:3.12-slim-trixie AS inference
RUN apt-get update && apt-get install -y --no-install-recommends build-essential cmake && rm -rf /var/lib/apt/lists/*
ENV CMAKE_BUILD_PARALLEL_LEVEL=2 CMAKE_ARGS="-DGGML_NATIVE=OFF -DGGML_OPENMP=OFF"
RUN pip wheel --no-cache-dir --wheel-dir=/wheels llama-cpp-python==0.3.16

FROM python:3.12-slim-trixie
COPY --from=inference /wheels /wheels
RUN pip install --no-index --find-links=/wheels llama-cpp-python==0.3.16 && rm -rf /wheels
COPY gil /usr/local/bin/gil
COPY model.gguf adapter.py latencies.csv /case/
RUN chmod 755 /case/adapter.py && mkdir /work && chown 65534:65534 /work
