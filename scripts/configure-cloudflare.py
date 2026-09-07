"""Provision Agent Monitor's Cloudflare Access app and tunnel route.

Usage:
    python scripts/configure-cloudflare.py prepare|publish

Required user environment variables:
    AGENT_MONITOR_PUBLIC_HOST
    AGENT_MONITOR_CF_ZONE
    AGENT_MONITOR_OWNER_EMAIL
    AGENT_MONITOR_CF_TEAM_DOMAIN
    AGENT_MONITOR_CF_TUNNEL_NAME
    AGENT_MONITOR_CF_API_TOKEN  (or CLOUDFLARE_API_TOKEN)

Credentials and deployment-specific identifiers are read from the local environment
and written only to Agent Monitor's ignored runtime data directory. They are never
printed or committed.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import urllib.error
import urllib.request
import winreg

ROOT = Path(__file__).resolve().parents[1]
DATA = Path(os.environ["LOCALAPPDATA"]) / "AgentMonitor"


def env(name: str) -> str:
    value = os.environ.get(name)
    if value:
        return value.strip()
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, "Environment") as key:
            return str(winreg.QueryValueEx(key, name)[0]).strip()
    except FileNotFoundError:
        return ""


def required(name: str) -> str:
    value = env(name)
    if not value:
        raise RuntimeError(f"{name} must be configured in the Windows user environment")
    return value


class Cloudflare:
    def __init__(self) -> None:
        self.token = env("AGENT_MONITOR_CF_API_TOKEN") or env("CLOUDFLARE_API_TOKEN")
        if not self.token:
            raise RuntimeError("A Cloudflare API token must be provided in the Windows user environment")

    def call(self, path: str, method: str = "GET", body: dict | None = None):
        request = urllib.request.Request(
            "https://api.cloudflare.com/client/v4" + path,
            method=method,
            headers={"Authorization": "Bearer " + self.token, "Content-Type": "application/json"},
            data=None if body is None else json.dumps(body).encode(),
        )
        try:
            with urllib.request.urlopen(request, timeout=25) as response:
                result = json.load(response)
        except urllib.error.HTTPError as error:
            # Never include request headers or returned service-token secrets in errors.
            raise RuntimeError(f"Cloudflare {method} {path}: HTTP {error.code}") from None
        if not result.get("success"):
            raise RuntimeError(f"Cloudflare {method} {path} failed: {result.get('errors')}")
        return result["result"]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["prepare", "publish"])
    args = parser.parse_args()

    host = required("AGENT_MONITOR_PUBLIC_HOST").lower()
    zone_name = required("AGENT_MONITOR_CF_ZONE").lower()
    owner = required("AGENT_MONITOR_OWNER_EMAIL").lower()
    team = required("AGENT_MONITOR_CF_TEAM_DOMAIN").lower()
    tunnel_name = required("AGENT_MONITOR_CF_TUNNEL_NAME")
    if not team.endswith(".cloudflareaccess.com"):
        raise RuntimeError("AGENT_MONITOR_CF_TEAM_DOMAIN must be a Cloudflare Access team domain")

    cf = Cloudflare()
    zones = cf.call(f"/zones?name={zone_name}")
    if len(zones) != 1:
        raise RuntimeError("Expected exactly one configured Cloudflare zone")
    zone = zones[0]
    account = zone["account"]["id"]
    prefix = f"/accounts/{account}"

    apps = cf.call(prefix + "/access/apps")
    candidates = [app for app in apps if app.get("domain") == host]
    if len(candidates) > 1:
        raise RuntimeError("Multiple Access applications own the configured Monitor hostname")
    app = candidates[0] if candidates else None
    if app and app.get("name") != "Agent Monitor":
        raise RuntimeError("Hostname belongs to another Access application; no changes made")

    if args.action == "prepare":
        if not app:
            app = cf.call(
                prefix + "/access/apps",
                "POST",
                {
                    "name": "Agent Monitor",
                    "domain": host,
                    "type": "self_hosted",
                    "session_duration": "168h",
                    "auto_redirect_to_identity": False,
                },
            )
        existing = cf.call(prefix + f"/access/apps/{app['id']}/policies")
        owner_policy = next((policy for policy in existing if policy.get("name") == "Agent Monitor owner"), None)
        body = {
            "name": "Agent Monitor owner",
            "decision": "allow",
            "include": [{"email": {"email": owner}}],
            "require": [],
            "exclude": [],
            "precedence": 1,
        }
        if owner_policy:
            cf.call(prefix + f"/access/apps/{app['id']}/policies/{owner_policy['id']}", "PUT", body)
        else:
            cf.call(prefix + f"/access/apps/{app['id']}/policies", "POST", body)

        DATA.mkdir(parents=True, exist_ok=True)
        (DATA / "remote.json").write_text(
            json.dumps({"publicUrl": "https://" + host, "teamDomain": team, "audience": app["aud"]}, indent=2),
            encoding="utf-8",
        )
        (DATA / "cloudflare-deployment.json").write_text(
            json.dumps(
                {"accountId": account, "zoneId": zone["id"], "appId": app["id"], "hostname": host, "ownerEmail": owner},
                indent=2,
            ),
            encoding="utf-8",
        )
        print("Prepared owner-only Agent Monitor Access application. Deployment identifiers remain local.")
        return

    if not app:
        raise RuntimeError("Prepare the Access application before publishing")
    policies = cf.call(prefix + f"/access/apps/{app['id']}/policies")
    if not any(policy.get("name") == "Agent Monitor owner" and policy.get("decision") == "allow" for policy in policies):
        raise RuntimeError("Owner policy missing; refusing to publish")
    if any(policy.get("decision") == "bypass" for policy in policies):
        raise RuntimeError("Unexpected bypass policy; refusing to publish")

    with urllib.request.urlopen("http://127.0.0.1:43218/api/setup", timeout=10) as response:
        setup = json.load(response)
    if setup.get("publicUrl") != "https://" + host:
        raise RuntimeError("Origin is not running the configured remote release")

    tunnels = cf.call(prefix + "/cfd_tunnel?is_deleted=false")
    tunnel = next((item for item in tunnels if item.get("name") == tunnel_name), None)
    if not tunnel:
        raise RuntimeError("Configured Cloudflare tunnel was not found")
    path = prefix + f"/cfd_tunnel/{tunnel['id']}/configurations"
    current = cf.call(path)
    config = current["config"]
    before = config.get("ingress", [])
    routes = [route for route in before if route.get("hostname") == host]
    if routes and (len(routes) != 1 or routes[0].get("service") != "http://127.0.0.1:43218"):
        raise RuntimeError("Monitor hostname already has a different route")
    if not routes:
        if not before or "hostname" in before[-1]:
            raise RuntimeError("Expected existing catch-all tunnel rule")
        (ROOT / ".local" / "cloudflare-ingress-before.json").parent.mkdir(parents=True, exist_ok=True)
        (ROOT / ".local" / "cloudflare-ingress-before.json").write_text(json.dumps(config, indent=2), encoding="utf-8")
        config["ingress"] = before[:-1] + [{"hostname": host, "service": "http://127.0.0.1:43218"}] + before[-1:]
        cf.call(path, "PUT", {"config": config})

    records = cf.call(f"/zones/{zone['id']}/dns_records?name={host}")
    target = tunnel["id"] + ".cfargotunnel.com"
    if records:
        if len(records) != 1 or records[0].get("content") != target or not records[0].get("proxied"):
            raise RuntimeError("Existing DNS differs; no overwrite performed")
    else:
        cf.call(
            f"/zones/{zone['id']}/dns_records",
            "POST",
            {"type": "CNAME", "name": host, "content": target, "proxied": True, "ttl": 1},
        )

    after = cf.call(path)["config"]["ingress"]
    if [route for route in after if route.get("hostname") != host] != [route for route in before if route.get("hostname") != host]:
        raise RuntimeError("An unrelated tunnel route changed")
    print("Published Agent Monitor through the configured Cloudflare hostname. Existing unrelated routes were preserved.")


if __name__ == "__main__":
    main()
