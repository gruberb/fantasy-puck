import { Outlet } from "react-router-dom";
import NavBar from "./NavBar";

const Layout = () => {
  return (
    <div className="min-h-screen flex flex-col bg-[#FAFAFA]">
      <NavBar />

      <main className="flex-grow container mx-auto max-w-7xl px-4 py-6 lg:py-8">
        <div className="animate-fadeIn"><Outlet /></div>
      </main>
      <footer className="bg-white text-[#1A1A1A] border-t-3 border-[#1A1A1A] p-6 text-center">
        <div className="items-center">
          <p className="text-gray-400 text-sm uppercase tracking-wide">
            Made with{' '}
            <span className="text-[#EC4899] inline-block">
              &#10084;
            </span>{' '}
            by{' '}
            <a
              href="https://bastiangruber.ca"
              target="_blank"
              rel="noopener noreferrer"
              className="text-[#2563EB] hover:text-[#1A1A1A] transition-colors duration-100
                             font-bold uppercase tracking-wider"
            >
              Bastian
            </a>
          </p>
        </div>
      </footer>

      {/* Add styles for animations */}
      <style>{`
        @keyframes fadeIn {
          from {
            opacity: 0;
            transform: translateY(10px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }

        .animate-fadeIn {
          animation: fadeIn 0.3s ease-out forwards;
        }
      `}</style>
    </div>
  );
};

export default Layout;
