import { useEffect } from "react";
import { Navigation } from "@/components/Navigation";
import { WebCliTerminal } from "@/components/WebCliTerminal";

const Cli = () => {
  useEffect(() => {
    document.title = "Chain CLI — COINjecture";
  }, []);

  return (
    <div className="min-h-screen bg-background">
      <Navigation />
      <main className="pt-24 pb-12 px-4 sm:px-6">
        <div className="container mx-auto max-w-4xl">
          <h1 className="sr-only">Chain CLI</h1>
          <WebCliTerminal />
        </div>
      </main>
    </div>
  );
};

export default Cli;
