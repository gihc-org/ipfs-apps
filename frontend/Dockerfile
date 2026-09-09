FROM nginx:1.27-alpine

COPY . /usr/share/nginx/html

EXPOSE 80

# config.js overskrives i k8s via ConfigMap-subPath-mount — ingen rebuild pr. miljø.
# TODO M2: nginx.conf med SPA-fallback hvis /h/:id routes til web-frontenden,
# og CSP/Permissions-Policy i app-laget (jf. MIGRATION.md).
