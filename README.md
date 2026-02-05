# wg_restarter

wireguard interface restarter.

Requires systemd. Additionally the wireguard connection must be managed via the `wg-quick@` service.

## usage

```
Usage: wg_restarter [OPTIONS] [INTERFACE]

Arguments:
  [INTERFACE]  WireGuard interface to monitor

Options:
  -t, --timeout <TIMEOUT>
          Handshake timeout in seconds [default: 10m]
  -l, --loop-interval <LOOP_INTERVAL>
          Loop interval in seconds [default: 60s]
  -r, --retry-after-unit-restart <RETRY_AFTER_UNIT_RESTART>
          Retry interval after unit restart in seconds [default: 30s]
```

## roadmap

* add support for configuration files
* add support for wireguard networks connected to via NetworkManager
* add systemd service file
