interface LoadingSpinnerProps {
  size?: "small" | "medium" | "large";
  message?: string;
}

const LoadingSpinner = ({
  size = "medium",
  message = "Loading...",
}: LoadingSpinnerProps) => {
  const sizeClasses = {
    small: "w-4 h-4 border-2",
    medium: "w-8 h-8 border-4",
    large: "w-12 h-12 border-4",
  };

  return (
    <div className="flex flex-col items-center justify-center py-8">
      <div
        className={`${sizeClasses[size]} border-[#1A1A1A] border-t-[#FACC15] rounded-full animate-spin`}
      />
      {message && <p className="mt-4 text-[#1A1A1A] font-bold uppercase tracking-wider text-sm">{message}</p>}
    </div>
  );
};

export default LoadingSpinner;
