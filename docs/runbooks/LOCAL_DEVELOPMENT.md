# Local development runbook

## Start sterile environment

```bash
make dev
make shell
```

## Check project

```bash
make full-check
```

## Start backend API

```bash
make api
```

API: http://localhost:8080/api/health

## Start UI

In another shell inside the container:

```bash
make ui-dev
```

UI: http://localhost:3000
