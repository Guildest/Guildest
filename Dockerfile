FROM python:3.11-slim AS runtime

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_NO_CACHE_DIR=1

WORKDIR /app

# System deps for common Python builds; keep slim.
RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential gcc \
    && rm -rf /var/lib/apt/lists/*

# Install Python dependencies (shared across services).
COPY requirements.txt /app/requirements.txt
RUN pip install -r /app/requirements.txt

# Copy application code.
COPY . /app

# Default command runs the Discord gateway; override with other entrypoints as needed.
CMD ["python", "-m", "backend.discord_gateway.main"]
