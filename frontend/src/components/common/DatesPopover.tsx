import { Link } from "react-router-dom";

const DatesPopover = ({
  dates,
  title,
  isOpen,
  onClose,
}: {
  dates: string[];
  title: string;
  isOpen: boolean;
  onClose: () => void;
}) => {
  if (!isOpen) return null;

  return (
    <div className="absolute z-50 w-64 p-3 bg-white rounded-none border border-gray-200 mt-1 -top-20 -left-52">
      <div className="flex justify-between items-center mb-2">
        <h4 className="font-medium text-sm">{title}</h4>
        <button onClick={onClose} className="text-gray-500 hover:text-gray-700">
          <svg
            className="w-4 h-4"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth={2}
              d="M6 18L18 6M6 6l12 12"
            />
          </svg>
        </button>
      </div>
      <div className="max-h-40 overflow-y-auto">
        {dates.length > 0 ? (
          <div className="flex flex-wrap gap-2">
            {dates.map((date) => (
              <Link
                key={date}
                to={`/games/${date}?tab=fantasy`}
                className="px-2 py-1 text-xs rounded hover:bg-gray-100 border border-gray-200 flex items-center"
              >
                {new Date(date).toLocaleDateString("en-US", {
                  month: "short",
                  day: "numeric",
                  timeZone: "UTC",
                })}
                <svg
                  className="w-3 h-3 ml-1"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M14 5l7 7m0 0l-7 7m7-7H3"
                  />
                </svg>
              </Link>
            ))}
          </div>
        ) : (
          <p className="text-gray-500 text-sm">No dates available</p>
        )}
      </div>
    </div>
  );
};

export default DatesPopover;
