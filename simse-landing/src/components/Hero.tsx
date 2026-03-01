import Typewriter from "./Typewriter";
import WaitlistForm from "./WaitlistForm";

export default function Hero() {
  return (
    <section className="flex flex-1 flex-col items-center justify-center px-5 pb-20 sm:px-6">
      <p
        className="animate-fade-in font-mono text-sm font-bold tracking-[0.35em] text-zinc-600"
        style={{ animationDelay: "50ms" }}
      >
        SIMSE-CODE
      </p>
      <h1
        className="animate-fade-in-up mt-6 text-center text-[1.625rem] leading-[1.2] font-bold tracking-[-0.02em] text-white min-[400px]:text-[1.875rem] sm:mt-8 sm:text-[3.5rem] sm:leading-[1.1] lg:text-[4rem]"
        style={{ animationDelay: "150ms" }}
      >
        <Typewriter /> assistant
        <br />
        that <span className="text-emerald-400">evolves</span> with you
      </h1>
      <p
        className="animate-fade-in-up mt-6 max-w-lg text-center text-base leading-relaxed tracking-[-0.01em] text-zinc-400 sm:text-lg"
        style={{ animationDelay: "300ms" }}
      >
        Use any ACP | MCP. Context carries over. Preferences stick.
        <br className="hidden sm:block" />
        {" "}An assistant that actually gets better the more you use it.
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
