FROM node:23.11.0-alpine AS builder

ARG VITE_API_HOST
ARG PROJECT_NAME

WORKDIR /app
COPY ./ui/${PROJECT_NAME} .
ENV VITE_API_HOST=${VITE_API_HOST}
RUN npm install --package-lock-only && npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html