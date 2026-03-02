import clsx from "clsx";
import SimseLogo from "./SimseLogo";
import Typewriter from "./Typewriter";
import WaitlistForm from "./WaitlistForm";

export default function Hero() {
  return (
    <section
      className={clsx(
        "flex flex-1 flex-col items-center justify-center px-5 sm:px-6",
      )}
    >
      <div
        className={clsx("animate-fade-in flex items-center gap-2.5")}
        style={{ animationDelay: "50ms" }}
      >
        <SimseLogo size={20} className="text-zinc-600" />
        <p className="font-mono text-sm font-bold tracking-[0.35em] text-zinc-600">
          SIMSE
        </p>
      </div>
      <h1
        className={clsx(
          "animate-fade-in-up mt-6 max-w-full overflow-hidden text-center text-[1.625rem] leading-[1.2] font-bold tracking-[-0.02em] text-white",
          "min-[400px]:text-[1.875rem] sm:mt-8 sm:text-[3.5rem] sm:leading-[1.1] lg:text-[4rem]",
        )}
        style={{ animationDelay: "150ms" }}
      >
        <Typewriter /> assistant
        <br />
        that <span className={clsx("text-emerald-400")}>evolves</span> with you
      </h1>
      <p
        className={clsx(
          "animate-fade-in-up mt-6 max-w-lg text-center text-base leading-relaxed tracking-[-0.01em] text-zinc-400 sm:text-lg",
        )}
        style={{ animationDelay: "300ms" }}
      >
        Use any ACP | MCP.
        <br />
        Context carries over. Preferences stick
        <br />
        An assistant that gets better the more you use it.
      </p>
      <div
        className={clsx("animate-fade-in-up mt-10 w-full max-w-lg")}
        style={{ animationDelay: "450ms" }}
      >
        <WaitlistForm />
      </div>
    </section>
  );
}
