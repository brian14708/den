import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "den",
  description: "Personal agent hub & dashboard",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="antialiased bg-neutral-950 text-neutral-100">
        {children}
      </body>
    </html>
  );
}
