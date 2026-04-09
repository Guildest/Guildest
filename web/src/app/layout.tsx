import type { Metadata } from "next";
import { Inter, Krona_One } from "next/font/google";
import "./globals.css";

const inter = Inter({
  variable: "--font-inter",
  subsets: ["latin"],
});

const kronaOne = Krona_One({
  variable: "--font-krona",
  weight: "400",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "Guildest - Build better Discord communities",
  description: "Guildest provides the right stats, so you could correctly improve your community",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={`${inter.variable} ${kronaOne.variable} font-sans antialiased`}>
        {children}
      </body>
    </html>
  );
}
