import Typewriter from "./Typewriter";
import WaitlistForm from "./WaitlistForm";

export default function Hero() {
  return (
    <section className="flex flex-1 flex-col items-center justify-center px-6 pb-20">
      <p
        className="animate-fade-in font-mono text-sm font-bold tracking-[0.35em] text-zinc-600"
        style={{ animationDelay: "50ms" }}
      >
        SIMSE-CODE
      </p>
      <h1
        className="animate-fade-in-up mt-8 text-center text-[2.75rem] leading-[1.1] font-bold tracking-[-0.025em] text-white sm:text-[3.5rem] lg:text-[4rem]"
        style={{ animationDelay: "150ms" }}
      >
        A <Typewriter /> assistant
        <br />
        that <span className="text-emerald-400">grows with you</span>
      </h1>
      <p
        className="animate-fade-in-up mt-6 max-w-lg text-center text-base leading-relaxed tracking-[-0.01em] text-zinc-400 sm:text-lg"
        style={{ animationDelay: "300ms" }}
      >
        Context that carries over. Preferences that stick.
        <br className="hidden sm:block" />
        An assistant that actually gets better the more you use it.
      </p>
      <div
        className="animate-fade-in-up mt-10 w-full max-w-lg"
        style={{ animationDelay: "450ms" }}
      >
        <WaitlistForm />
      </div>
    </section>
  );
}
