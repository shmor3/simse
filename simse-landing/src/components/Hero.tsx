import Typewriter from "./Typewriter";
import WaitlistForm from "./WaitlistForm";

export default function Hero() {
  return (
    <section className="flex flex-1 flex-col items-center justify-center px-6 pb-20">
      {/* Wordmark */}
      <p
        className="animate-fade-in font-mono text-xs font-bold tracking-[0.35em] text-zinc-600"
        style={{ animationDelay: "50ms" }}
      >
        SIMSE-CODE
      </p>

      {/* Headline */}
      <h1
        className="animate-fade-in-up mt-7 text-center text-[2.5rem] leading-[1.1] font-bold tracking-[-0.025em] text-white sm:text-[3.25rem]"
        style={{ animationDelay: "150ms" }}
      >
        The <Typewriter /> assistant
        <br />
        that <span className="text-emerald-400">grows with you</span>
      </h1>

      {/* Description */}
      <p
        className="animate-fade-in-up mt-5 max-w-md text-center text-[15px] leading-relaxed tracking-[-0.01em] text-zinc-400"
        style={{ animationDelay: "300ms" }}
      >
        Persistent memory that learns your patterns,
        <br className="hidden sm:block" />
        recalls past solutions, and gets smarter
        <br className="hidden sm:block" />
        every session.
      </p>

      {/* Form */}
      <div
        className="animate-fade-in-up mt-9 w-full max-w-lg"
        style={{ animationDelay: "450ms" }}
      >
        <WaitlistForm />
      </div>
    </section>
  );
}
