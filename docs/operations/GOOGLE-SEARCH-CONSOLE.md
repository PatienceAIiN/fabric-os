# Google Search Console setup

The website now includes the technical files needed for indexing:

- `website/public/robots.txt` allows public pages and excludes API/download paths.
- `website/public/sitemap.xml` lists the public Fabric OS routes.
- `website/index.html` includes canonical, Open Graph, Twitter, and crawler metadata.

## Owner action after DNS is connected

1. Open Google Search Console and add a **Domain property** for `fabricos.patienceai.in`.
2. Add the TXT record Google provides at the DNS provider for `patienceai.in`.
3. Wait for DNS propagation, then click Verify.
4. Submit `https://fabricos.patienceai.in/sitemap.xml`.
5. Inspect the home page and request indexing after the HTTPS certificate is active.

Do not publish a made-up verification token in source control. The token must be generated for the verified Google account and domain. Search Console ownership and indexing are external Google/DNS operations; code alone cannot complete them.
