//! `GET /healthz` — liveness til k8s-probes.

pub async fn healthz() -> &'static str {
    "ok"
}
