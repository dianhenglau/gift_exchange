# Initialize new build stage and set base image to rust:latest.
FROM rust:latest as builder

# Set working directory in the image. Create if not exists.
WORKDIR /usr/src

# Create new project.
RUN USER=root cargo new gift_exchange

# Change working directory.
WORKDIR /usr/src/gift_exchange

# Copy cargo files.
COPY Cargo.toml Cargo.lock ./

# Build the dependencies. We include this phase so that docker can cache it, and
# it can start from here next time we build this image.
RUN cargo build --release

# Copy folders from current directory into the image.
COPY src ./src
COPY templates ./templates

# Build the application. This time build the full application.
RUN cargo build --release

# Now initialize deployment stage, using cc-debian10 as base image.
FROM gcr.io/distroless/cc-debian10

# Change working directory. Create if not exists.
WORKDIR /usr/src/gift_exchange

# Copy binary and resources.
COPY --from=builder /usr/src/gift_exchange/target/release/hello ./target/release/hello
COPY static ./static
COPY data ./data

# Command to be run after the container is created.
CMD ["/usr/src/gift_exchange/target/release/hello"]
