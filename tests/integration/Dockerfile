FROM ubuntu:22.04

RUN apt-get update && apt-get install -y \
    curl \
    ca-certificates \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

WORKDIR /flockfly
COPY . .

RUN cargo build --release
RUN cp target/release/flockfly /usr/local/bin/flockfly

COPY tests/integration/run.sh /run-tests.sh
RUN chmod +x /run-tests.sh

CMD ["/run-tests.sh"]
