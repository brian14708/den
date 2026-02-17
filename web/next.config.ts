import type { NextConfig } from "next";

const isDev = process.env.NODE_ENV === "development";

const nextConfig: NextConfig = {
  output: "export",
  trailingSlash: true,
  ...(isDev && {
    rewrites: async () => [
      {
        source: "/api/:path*",
        destination: "http://localhost:3000/api/:path*",
      },
    ],
  }),
};

export default nextConfig;
