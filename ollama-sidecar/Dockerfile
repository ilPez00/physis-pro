# Ollama sidecar — serves the uncensored Gemma floor for the physis provider
# cascade, isolated from the engine's memory. Point physis at it with
#   OLLAMA_FLOOR_URL=http://<this-service>.railway.internal:11434/v1
#   OLLAMA_FLOOR_MODEL=gemma-uncensored
FROM ollama/ollama:latest

# curl for an HTTPS GGUF (e.g. the HuggingFace resolve URL). (S3/bucket sourcing
# would also need awscli, but we pull from a public URL.)
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*

# Accept connections from other Railway services (not just localhost).
ENV OLLAMA_HOST=0.0.0.0:11434
# Keep the model resident so the first cascade hit isn't a cold load.
ENV OLLAMA_KEEP_ALIVE=-1

COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

EXPOSE 11434
ENTRYPOINT ["/entrypoint.sh"]
