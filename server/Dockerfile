FROM rust:1.86

WORKDIR /app
COPY . .

RUN cargo install --path .

CMD ["ngpr-server"]