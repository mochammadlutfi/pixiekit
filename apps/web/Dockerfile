# syntax=docker/dockerfile:1.7
# ---------------------------------------------------------------------------
# Pixiekit web (Nuxt 4) — multi-stage build
#
# Build:
#   docker build -t pixiekit/web ./apps/web
#
# The runtime is configured via NUXT_PUBLIC_PIXIEKIT_API_URL (overrides
# runtimeConfig.public.pixiekitApiUrl from nuxt.config.ts at runtime).
# ---------------------------------------------------------------------------
ARG NODE_VERSION=20
ARG PNPM_VERSION=9

# ---- base ----------------------------------------------------------------
FROM node:${NODE_VERSION}-alpine AS base
ARG PNPM_VERSION
RUN apk add --no-cache libc6-compat \
 && npm install -g pnpm@${PNPM_VERSION}
WORKDIR /app

# ---- deps (cache-friendly, postinstall skipped) --------------------------
FROM base AS deps
COPY package.json pnpm-lock.yaml ./
RUN --mount=type=cache,id=pixiekit-pnpm,target=/root/.local/share/pnpm/store \
    pnpm install --frozen-lockfile --ignore-scripts

# ---- builder -------------------------------------------------------------
FROM base AS builder
ENV NODE_ENV=production \
    NUXT_TELEMETRY_DISABLED=1
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN pnpm build

# ---- runtime -------------------------------------------------------------
FROM node:${NODE_VERSION}-alpine AS runtime
RUN apk add --no-cache tini \
 && addgroup -g 1001 -S nodejs \
 && adduser  -u 1001 -S nuxt -G nodejs
WORKDIR /app
ENV NODE_ENV=production \
    HOST=0.0.0.0 \
    PORT=3000 \
    NITRO_PORT=3000 \
    NUXT_TELEMETRY_DISABLED=1
COPY --from=builder --chown=nuxt:nodejs /app/.output ./.output
USER nuxt
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -qO- http://127.0.0.1:${PORT:-3000}/ >/dev/null 2>&1 || exit 1
ENTRYPOINT ["/sbin/tini", "--"]
CMD ["node", ".output/server/index.mjs"]
