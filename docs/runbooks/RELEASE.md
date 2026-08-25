# Release runbook

1. Run `make full-check` inside the sterile container.
2. Build production image:

```bash
docker build -f Dockerfile.production -t quant-system:<version> .
```

3. Start with a persistent runs volume:

```bash
docker compose -f docker-compose.prod.yml up -d
```

4. Verify:

```bash
curl http://localhost:8080/api/health
```

5. Archive release manifest including git commit, image digest and config checksums.
