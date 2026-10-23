# Farm-Proxy deployment package

English translation of the Russian README at commit `9790a8f53d002f886e5d491c202af64682cc7045` in `hashcore-llc/stratum-proxy`. Scope: complete README body and configuration comments. Original files are in artifact `hashcore-proxy-deployment`. Date: 2026-10-19.

This is a translation of deployment instructions. The catalogue did not run these commands or containers. The repository contains deployment configuration and dashboards. It does not establish that the proxy application source is open source.

## Components

The package can start:

- Farm-Proxy, to proxy mining traffic.
- Prometheus, to aggregate metrics.
- NodeExporter, to collect metrics and send them to Prometheus.
- Grafana, to display graphs.

## System requirements

The README prefers Ubuntu 20.04. It recommends one of these hosts:

1. A 64-bit Intel, AMD or ARM processor with Linux. It names CentOS 7 or later, Ubuntu 16.04 or later, and compatible distributions.
2. Raspberry Pi 3, 4 or 5 with ARM64/ARMv8. It names 64-bit Raspberry Pi OS, Ubuntu 20.04 LTS and Ubuntu 22.04 LTS. The source says to install the OS with Raspberry Pi Imager.
3. Windows 10 or 11, 64-bit, with WSL2, Ubuntu 20.04 LTS and hardware virtualization enabled in BIOS/UEFI.

Recommended capacity is 2 GB RAM and a 64-bit Intel, AMD or ARM processor. These are source recommendations, not benchmark results from this catalogue.

## Installation described by the source

1. Install Docker and Docker Compose. The source links to https://docs.docker.com/engine/install/.
2. Install Git. The source links to the Git installation guide.
3. Run `git clone git@bitbucket.org:anthill-farm/farm-proxy-package.git`.
4. Change to directory `farm-proxy-package`.
5. On Windows, change `./configs` to `.\configs` in the `volume` section of `docker-compose.yml`.
6. Run `docker compose up -d`.
7. Open `http://localhost:5000` in a browser.

Translator note: the clone URL above is the literal older Bitbucket URL in the README. The archived public source is the GitHub repository named at the start of this translation. The source's startup section repeats `docker compose up -d` from the project root.

## Configuration described by the source

The README names `configs/farm-proxy.json` and `configs/proxy_ports.json`, then shows YAML examples with different names. The current archived tree instead contains `configs/stratum-proxy.yaml` and `configs/configurator.yaml`. These naming conflicts are retained. Do not use the old names as evidence of the current file interface.

The first example, labelled `farm-proxy.yaml`, is:

```yaml
log_level: info
mining_algorithm: sha256d
bind_api_addr: 0.0.0.0:5010
log_dir: "./configs/log"
```

The second example, labelled `farm-proxy_ports.yaml`, is:

```yaml
- bind_port: 4042
  # Ratio used to set each miner's difficulty.
  # 0.2 shares per second = 1 share / 5 seconds.
  submission_rate: 0.2
  mining_algorithm: scrypt
  pool:
    # Pool address
    url: btc.viabtc.io:3333
    # Account name
    user: login
    # Password
    pass: password
- bind_port: 4043
  submission_rate: 0.2
  mining_algorithm: sha256d
  pool:
    url: btc.viabtc.io:3333
    user: login
    pass: password
```

The source includes a Scrypt example. It is retained as part of the translation and is outside the main Bitcoin ASIC scope. The first pool example's algorithm and BTC endpoint also require review before use.

Prometheus settings are in `monitoring/prometheus/alert.rules` and `monitoring/prometheus/prometheus.yml`.

## Logs

Farm-Proxy logs are in `configs/log`.
