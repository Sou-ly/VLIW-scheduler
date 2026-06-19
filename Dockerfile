# Reproducible build/test environment for the VLIW-470 scheduler.
#   docker build -t vliw470 .
#   docker run -it -v "$(pwd)":/work vliw470
# Inside the container:
#   ./runall.sh && ./testall.sh
FROM rust:1.78-slim

# Python is only needed for compare.py (the schedule checker).
RUN apt-get update \
    && apt-get install -y --no-install-recommends python3 python-is-python3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work
COPY . .

RUN cargo build --release

CMD ["/bin/bash"]
