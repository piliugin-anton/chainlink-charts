import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";

import { QueryProvider } from "@/components/QueryProvider";

import "./globals.css";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "Chainlink live charts",
  description:
    "BTC, ETH, SOL, XRP — Chainlink Data Streams Candlestick API via Next.js BFF",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${geistSans.variable} ${geistMono.variable} h-full antialiased`}
    >
      <body className="min-h-full bg-zinc-950 text-zinc-100">
        <QueryProvider>{children}</QueryProvider>
      </body>
    </html>
  );
}
