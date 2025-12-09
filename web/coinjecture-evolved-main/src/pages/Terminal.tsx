import { Navigation } from "@/components/Navigation";
import { Footer } from "@/components/Footer";
import { Terminal as TerminalComponent } from "@/components/Terminal";

const Terminal = () => {
  return (
    <div className="min-h-screen">
      <Navigation />
      <main className="pt-32 pb-20">
        <div className="container mx-auto px-6">
          <div className="max-w-6xl mx-auto">
            <div className="text-center mb-12 animate-fade-in">
              <h1 className="text-4xl md:text-5xl font-bold mb-4">
                Interactive <span className="text-primary">Terminal</span>
              </h1>
              <p className="text-lg text-muted-foreground max-w-2xl mx-auto">
                Mine $BEANS through computational contributions. Try commands like 'help', 'mine start', or 'wallet balance'.
              </p>
            </div>
            <TerminalComponent />
          </div>
        </div>
      </main>
      <Footer />
    </div>
  );
};

export default Terminal;
