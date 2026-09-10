import express from 'express';
import pg from 'pg';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const app = express();
const port = process.env.PORT || 4173;
const __dirname = path.dirname(fileURLToPath(import.meta.url));
const pool = process.env.DATABASE_URL ? new pg.Pool({
  connectionString: process.env.DATABASE_URL,
  ssl: process.env.DATABASE_SSL === 'true' ? { rejectUnauthorized: false } : false,
}) : null;
const rateWindowMs = 60_000;
const rateLimit = 30;
const requestCounts = new Map();
const apiRateLimit = (req, res, next) => {
  const now = Date.now();
  const key = req.ip || 'unknown';
  const current = requestCounts.get(key);
  const entry = current && current.resetAt > now ? current : { count: 0, resetAt: now + rateWindowMs };
  entry.count += 1;
  requestCounts.set(key, entry);
  if (requestCounts.size > 1000) for (const [ip, item] of requestCounts) if (item.resetAt <= now) requestCounts.delete(ip);
  if (entry.count > rateLimit) return res.status(429).json({ error: 'Too many requests' });
  next();
};

app.disable('x-powered-by');
app.set('trust proxy', 1);
app.use((_req, res, next) => {
  res.setHeader('X-Content-Type-Options', 'nosniff');
  res.setHeader('X-Frame-Options', 'DENY');
  res.setHeader('Referrer-Policy', 'strict-origin-when-cross-origin');
  res.setHeader('Permissions-Policy', 'camera=(), microphone=(), geolocation=()');
  res.setHeader('Cross-Origin-Opener-Policy', 'same-origin');
  res.setHeader('Cross-Origin-Resource-Policy', 'same-origin');
  if (process.env.NODE_ENV === 'production') res.setHeader('Strict-Transport-Security', 'max-age=31536000; includeSubDomains');
  res.setHeader('Content-Security-Policy', "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; img-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'");
  next();
});
app.use(express.json({ limit: '10kb' }));
app.use('/api/', apiRateLimit);
app.get('/healthz', (_req, res) => res.status(200).json({ ok: true }));
app.post('/api/downloads', async (req, res) => {
  const platform = typeof req.body?.platform === 'string' ? req.body.platform.slice(0, 32) : '';
  if (!/^[a-z0-9_-]{1,32}$/.test(platform)) return res.status(400).json({ accepted: false });
  if (pool) {
    try { await pool.query('INSERT INTO download_events (platform) VALUES ($1)', [platform]); } catch { return res.status(503).json({ accepted: false }); }
  }
  res.status(202).json({ accepted: true });
});
app.post('/api/consent', async (req, res) => {
  const policyVersion = typeof req.body?.policyVersion === 'string' ? req.body.policyVersion.slice(0, 40) : 'unknown';
  const preferences = req.body?.preferences || {};
  const necessary = preferences.necessary === true;
  const analytics = preferences.analytics === true;
  if (pool) {
    try {
      await pool.query('INSERT INTO consent_events (policy_version, necessary, analytics) VALUES ($1, $2, $3)', [policyVersion, necessary, analytics]);
    } catch {
      return res.status(503).json({ accepted: false });
    }
  }
  res.status(202).json({ accepted: true });
});
app.get('/downloads/fabric-os-0.1.0-x86_64.img', (_req, res) => {
  res.download(path.join(__dirname, '../../build/rootfs.ext4'), 'fabric-os-0.1.0-x86_64.img');
});
app.use(express.static(path.join(__dirname, '../dist')));
app.use((_req, res) => res.sendFile(path.join(__dirname, '../dist/index.html')));
app.listen(port, () => console.log(`Fabric OS site listening on ${port}`));
