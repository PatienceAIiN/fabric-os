# Fabric OS website

Public product site for Fabric OS by Patience AI. The site is intentionally product-facing: it describes the principles and supported environments without exposing internal implementation details or credentials.

## Run locally

```bash
npm install
npm run dev
```

For the production-style server:

```bash
npm run build
node server/index.js
```

Set `DATABASE_URL` to enable PostgreSQL-backed download and consent event storage, then apply `db/schema.sql`. Without a database, the API still accepts events so the public site remains usable in local preview. Optional analytics remains off by default.

## Routes

- `/` — overview
- `/architecture` — public architecture story
- `/security` — safety principles
- `/download` — supported environments and x86_64 download
- `/docs` — concise public documentation
- `/device` — physical-device evaluation guide
- `/privacy` — public privacy notice starter
- `/terms` — early-build terms starter
- `/licenses` — license and dependency notices
- `/compliance` — public launch-readiness summary

Launch compliance material lives in `../docs/compliance/`. It intentionally contains explicit launch blockers for the legal entity, jurisdiction, vendors, retention, transfers, and counsel approval. Do not present the site as GDPR-certified or legally complete until those items have been completed and approved.
