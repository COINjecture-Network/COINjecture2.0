COINjecture API on a Linux VPS (e.g. Hostinger)
================================================

Cursor/AI cannot SSH into your server. You (or support) must run commands on the VPS.

1) SSH:  ssh YOUR_USER@YOUR_SERVER_IP

2) Upload this folder or clone the full repo on the VPS.

3) Copy api.env.example to /etc/coinjecture/api.env, fill SUPABASE_* from Supabase dashboard:
     sudo mkdir -p /etc/coinjecture
     sudo nano /etc/coinjecture/api.env
     sudo chmod 600 /etc/coinjecture/api.env

4) Build + install API binary (or use Docker — see api-server/README.md):
     cargo build --release -p coinjecture-api-server
     sudo install -m 755 target/release/coinjecture-api /opt/coinjecture/coinjecture-api
     Install coinjecture-api.service from this folder, then: sudo systemctl enable --now coinjecture-api

5) DNS: A record api.coinjecture.com -> server IP. Then TLS:
     sudo certbot certonly --nginx -d api.coinjecture.com
     Use nginx-api.conf.example behind sites-enabled (edit SSL paths if needed).

6) Frontend: VITE_API_URL=https://api.coinjecture.com in .env.production, rebuild, deploy S3.
