FROM node:23.11.0-alpine AS builder

ARG VITE_API_HOST

WORKDIR /app
COPY . .
ENV VITE_API_HOST=${VITE_API_HOST}
RUN npm install && npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
CMD ["nginx", "-g", "daemon off;"]