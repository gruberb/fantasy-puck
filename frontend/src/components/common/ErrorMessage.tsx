interface ErrorMessageProps {
  message?: string;
  onRetry?: () => void;
}

const ErrorMessage = ({
  message = "An error occurred. Please try again.",
  onRetry,
}: ErrorMessageProps) => {
  return (
    <div className="bg-[#EF4444]/10 border-2 border-[#EF4444] rounded-none text-red-700 px-4 py-3 my-4">
      <p className="font-bold uppercase tracking-wider text-sm">{message}</p>

      {onRetry && (
        <button
          onClick={onRetry}
          className="mt-2 bg-[#EF4444] text-white border-2 border-[#1A1A1A] rounded-none px-3 py-1 text-sm font-bold uppercase tracking-wider shadow-[3px_3px_0px_0px_#1A1A1A] hover:translate-x-[2px] hover:translate-y-[2px] hover:shadow-none transition-all duration-100"
        >
          Try Again
        </button>
      )}
    </div>
  );
};

export default ErrorMessage;
