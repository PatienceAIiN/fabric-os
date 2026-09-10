# Fabric OS website data map

## Current website flow

| Flow | Data | Purpose | Storage | Decision needed |
| --- | --- | --- | --- | --- |
| Download click | `platform` (`x86_64`, etc.) | Count release demand and diagnose abuse | PostgreSQL, if `DATABASE_URL` is configured | Approve retention period and access owners |
| Download click | `created_at` | Event timing and release operations | PostgreSQL, if configured | Approve retention period |
| Hosting/runtime | Provider-defined request/security logs | Availability, abuse prevention, debugging | Hosting provider | Identify provider, fields, region, retention, DPA |
| Contact email | Email contents and metadata | Respond to support/privacy requests | Mail provider | Identify provider, retention, access, DPA |

The website should not intentionally collect names, prompts, files, API keys, model outputs, or advertising identifiers through the public download flow.
