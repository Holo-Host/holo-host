FROM node:23.11.0-alpine AS builder

WORKDIR /app
COPY . .
RUN npm install --package-lock-only && npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html