import { ReactNode } from "react";

interface PageHeaderProps {
  title: string;
  subtitle?: string;
  badge?: string;
  children?: ReactNode;
}

const PageHeader = ({ title, subtitle, badge, children }: PageHeaderProps) => {
  return (
    <div className="bg-white rounded-none p-6 mb-6">
      <div className="flex flex-col sm:flex-row justify-between sm:items-start gap-3">
        <div>
          <h1>{title}</h1>
          {subtitle && (
            <p className="text-sm text-gray-500 mt-1">{subtitle}</p>
          )}
          {badge && (
            <span className="inline-block mt-2 px-2 py-0.5 text-xs uppercase tracking-wider bg-[#FACC15]">
              {badge}
            </span>
          )}
        </div>
        {children && <div>{children}</div>}
      </div>
    </div>
  );
};

export default PageHeader;
