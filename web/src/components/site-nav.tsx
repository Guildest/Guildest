import Image from "next/image";
import Link from "next/link";

export function SiteNav() {
  return (
    <nav className="sticky top-0 z-50 bg-plum/80 backdrop-blur-md border-b border-border-light/20">
      <div className="relative max-w-7xl mx-auto px-6 h-16 flex items-center justify-between">
        <Link href="/" className="flex items-center">
          <Image src="/logolanding.svg" alt="Guildest" width={28} height={26} />
        </Link>
        <div className="hidden md:flex absolute left-1/2 -translate-x-1/2 items-center gap-6 text-[14px] text-cream/65">
          <Link href="/teams" className="hover:text-cream transition-colors">Teams</Link>
        </div>
        <div className="flex items-center gap-2">
          <Link
            href="/teams#contact"
            className="text-[14px] text-cream/70 hover:text-cream px-3 py-1.5 transition-colors"
          >
            Book a demo
          </Link>
          <Link
            href="/waitlist"
            className="text-[14px] text-plum bg-cream hover:bg-cream/90 px-4 py-2 transition-colors font-medium"
          >
            Join waitlist
          </Link>
        </div>
      </div>
    </nav>
  );
}
