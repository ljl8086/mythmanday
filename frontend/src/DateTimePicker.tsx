import React, { useState, useRef, useEffect } from "react";
import { Calendar as CalendarIcon, Clock, ChevronLeft, ChevronRight } from "lucide-react";

interface ModernDatePickerProps {
  value: string; // "YYYY-MM-DD"
  onChange: (date: string) => void;
  label?: string;
  className?: string;
}

export const ModernDatePicker: React.FC<ModernDatePickerProps> = ({
  value,
  onChange,
  label = "工作日期",
  className = "",
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // 解析当前选中的年月日
  const parsedDate = value ? new Date(value + "T00:00:00") : new Date();
  const [viewYear, setViewYear] = useState(parsedDate.getFullYear());
  const [viewMonth, setViewMonth] = useState(parsedDate.getMonth()); // 0-11

  useEffect(() => {
    if (value) {
      const d = new Date(value + "T00:00:00");
      if (!isNaN(d.getTime())) {
        setViewYear(d.getFullYear());
        setViewMonth(d.getMonth());
      }
    }
  }, [value]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  const handlePrevMonth = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (viewMonth === 0) {
      setViewMonth(11);
      setViewYear(viewYear - 1);
    } else {
      setViewMonth(viewMonth - 1);
    }
  };

  const handleNextMonth = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (viewMonth === 11) {
      setViewMonth(0);
      setViewYear(viewYear + 1);
    } else {
      setViewMonth(viewMonth + 1);
    }
  };

  const setQuickDate = (offsetDays: number) => {
    const target = new Date();
    target.setDate(target.getDate() + offsetDays);
    const y = target.getFullYear();
    const m = String(target.getMonth() + 1).padStart(2, "0");
    const d = String(target.getDate()).padStart(2, "0");
    onChange(`${y}-${m}-${d}`);
    setIsOpen(false);
  };

  // 生成日历网格
  const firstDayOfWeek = new Date(viewYear, viewMonth, 1).getDay(); // 0(Sun) - 6(Sat)
  // 中国惯用周一作为每周第一天 (0:Mon, ..., 6:Sun)
  const mondayOffset = (firstDayOfWeek + 6) % 7;
  const daysInCurrentMonth = new Date(viewYear, viewMonth + 1, 0).getDate();
  const daysInPrevMonth = new Date(viewYear, viewMonth, 0).getDate();

  const days: { day: number; isCurrentMonth: boolean; dateStr: string }[] = [];

  // 上月占位日
  for (let i = mondayOffset - 1; i >= 0; i--) {
    const day = daysInPrevMonth - i;
    const prevMonthIdx = viewMonth === 0 ? 11 : viewMonth - 1;
    const prevYear = viewMonth === 0 ? viewYear - 1 : viewYear;
    const dateStr = `${prevYear}-${String(prevMonthIdx + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
    days.push({ day, isCurrentMonth: false, dateStr });
  }

  // 当月天数
  for (let i = 1; i <= daysInCurrentMonth; i++) {
    const dateStr = `${viewYear}-${String(viewMonth + 1).padStart(2, "0")}-${String(i).padStart(2, "0")}`;
    days.push({ day: i, isCurrentMonth: true, dateStr });
  }

  // 下月补齐到 35 或 42 格
  const remaining = (7 - (days.length % 7)) % 7;
  for (let i = 1; i <= remaining; i++) {
    const nextMonthIdx = viewMonth === 11 ? 0 : viewMonth + 1;
    const nextYear = viewMonth === 11 ? viewYear + 1 : viewYear;
    const dateStr = `${nextYear}-${String(nextMonthIdx + 1).padStart(2, "0")}-${String(i).padStart(2, "0")}`;
    days.push({ day: i, isCurrentMonth: false, dateStr });
  }

  // 格式化展示：例如 "2026-10-12 星期一"
  const getDisplayLabel = () => {
    if (!value) return "请选择工作日期";
    const d = new Date(value + "T00:00:00");
    if (isNaN(d.getTime())) return value;
    const weekMap = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];
    const week = weekMap[d.getDay()];
    
    // 相对今天判断
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const diffTime = d.getTime() - today.getTime();
    const diffDays = Math.round(diffTime / (1000 * 3600 * 24));
    let relTag = "";
    if (diffDays === 0) relTag = " (今天)";
    else if (diffDays === -1) relTag = " (昨天)";
    else if (diffDays === -2) relTag = " (前天)";

    return `${value} ${week}${relTag}`;
  };

  const todayStr = (() => {
    const t = new Date();
    return `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, "0")}-${String(t.getDate()).padStart(2, "0")}`;
  })();

  return (
    <div className={`relative ${className}`} ref={containerRef}>
      {label && <label className="text-xs font-bold text-slate-700 block mb-1">{label}</label>}

      {/* 触发器按键 */}
      <button
        type="button"
        onClick={() => setIsOpen(!isOpen)}
        className={`w-full h-9 px-3 rounded-xl border bg-white flex items-center justify-between text-xs transition-all cursor-pointer ${
          isOpen
            ? "border-indigo-500 ring-2 ring-indigo-500/20 text-indigo-900 font-semibold shadow-xs"
            : "border-slate-200 hover:border-slate-300 text-slate-800"
        }`}
      >
        <span className="flex items-center gap-2 font-mono">
          <CalendarIcon className="w-3.5 h-3.5 text-indigo-500" />
          <span>{getDisplayLabel()}</span>
        </span>
        <span className="text-[10px] text-indigo-600 bg-indigo-50 px-1.5 py-0.5 rounded font-sans">
          日历
        </span>
      </button>

      {/* 现代悬浮面板 */}
      {isOpen && (
        <div className="absolute top-full left-0 mt-1.5 w-72 bg-white rounded-2xl border border-slate-200/90 shadow-2xl p-3 z-60 animate-in fade-in zoom-in-95 duration-150">
          {/* 快捷今天 / 昨天选择器 */}
          <div className="flex items-center gap-1.5 pb-2.5 mb-2.5 border-b border-slate-100">
            <button
              type="button"
              onClick={() => setQuickDate(0)}
              className="flex-1 py-1 px-2 rounded-lg bg-indigo-50/70 hover:bg-indigo-100 text-indigo-700 text-[11px] font-semibold transition-all cursor-pointer"
            >
              今天
            </button>
            <button
              type="button"
              onClick={() => setQuickDate(-1)}
              className="flex-1 py-1 px-2 rounded-lg bg-slate-50 hover:bg-slate-100 text-slate-600 text-[11px] font-medium transition-all cursor-pointer"
            >
              昨天
            </button>
            <button
              type="button"
              onClick={() => setQuickDate(-2)}
              className="flex-1 py-1 px-2 rounded-lg bg-slate-50 hover:bg-slate-100 text-slate-600 text-[11px] font-medium transition-all cursor-pointer"
            >
              前天
            </button>
          </div>

          {/* 年月翻页切换 */}
          <div className="flex items-center justify-between px-1 mb-2">
            <span className="text-xs font-bold text-slate-800 font-mono">
              {viewYear}年 {viewMonth + 1}月
            </span>
            <div className="flex items-center gap-1">
              <button
                type="button"
                onClick={handlePrevMonth}
                className="w-6 h-6 flex items-center justify-center rounded-md hover:bg-slate-100 text-slate-500 hover:text-slate-800 transition-colors cursor-pointer"
              >
                <ChevronLeft className="w-3.5 h-3.5" />
              </button>
              <button
                type="button"
                onClick={handleNextMonth}
                className="w-6 h-6 flex items-center justify-center rounded-md hover:bg-slate-100 text-slate-500 hover:text-slate-800 transition-colors cursor-pointer"
              >
                <ChevronRight className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          {/* 星期表头 */}
          <div className="grid grid-cols-7 text-center text-[10px] font-bold text-slate-400 mb-1">
            <span>一</span>
            <span>二</span>
            <span>三</span>
            <span>四</span>
            <span>五</span>
            <span className="text-rose-400">六</span>
            <span className="text-rose-400">日</span>
          </div>

          {/* 日期数字矩阵 */}
          <div className="grid grid-cols-7 gap-1 text-center text-xs">
            {days.map((d, idx) => {
              const isSelected = d.dateStr === value;
              const isToday = d.dateStr === todayStr;

              return (
                <button
                  key={idx}
                  type="button"
                  onClick={() => {
                    onChange(d.dateStr);
                    setIsOpen(false);
                  }}
                  className={`h-7 rounded-lg flex items-center justify-center font-mono text-[11px] transition-all cursor-pointer ${
                    isSelected
                      ? "bg-indigo-600 text-white font-bold shadow-xs scale-105"
                      : isToday
                      ? "bg-indigo-50 text-indigo-700 font-bold border border-indigo-200/80"
                      : d.isCurrentMonth
                      ? "text-slate-700 hover:bg-slate-100 hover:text-slate-900"
                      : "text-slate-300 hover:bg-slate-50"
                  }`}
                >
                  {d.day}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
};

interface ModernTimePickerProps {
  value: string; // "HH:MM" (24小时制)
  onChange: (time: string) => void;
  label?: string;
  className?: string;
}

export const ModernTimePicker: React.FC<ModernTimePickerProps> = ({
  value,
  onChange,
  label,
  className = "",
}) => {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // 解析时与分
  const [hour, minute] = (() => {
    if (!value || !value.includes(":")) return ["09", "30"];
    const [h, m] = value.split(":");
    return [h.padStart(2, "0"), m.padStart(2, "0")];
  })();

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };
    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  const selectHour = (h: string) => {
    onChange(`${h}:${minute}`);
  };

  const selectMinute = (m: string) => {
    onChange(`${hour}:${m}`);
    setIsOpen(false);
  };

  // 快捷常用时间点
  const quickTimes = ["09:00", "09:30", "12:00", "13:30", "14:00", "18:00", "18:30", "20:00"];

  const hoursList = Array.from({ length: 24 }, (_, i) => String(i).padStart(2, "0"));
  const minutesList = ["00", "05", "10", "15", "20", "25", "30", "35", "40", "45", "50", "55"];

  return (
    <div className={`relative ${className}`} ref={containerRef}>
      {label && <label className="text-xs font-bold text-slate-700 block mb-1">{label}</label>}

      {/* 触发输入框 */}
      <div
        onClick={() => setIsOpen(!isOpen)}
        className={`w-full h-9 px-3 rounded-xl border bg-white flex items-center justify-between text-xs transition-all cursor-pointer ${
          isOpen
            ? "border-indigo-500 ring-2 ring-indigo-500/20 text-indigo-900 font-semibold shadow-xs"
            : "border-slate-200 hover:border-slate-300 text-slate-800"
        }`}
      >
        <div className="flex items-center gap-2 font-mono">
          <Clock className="w-3.5 h-3.5 text-indigo-500" />
          <span className="font-bold text-slate-800">{hour}</span>
          <span className="text-slate-400">:</span>
          <span className="font-bold text-slate-800">{minute}</span>
        </div>
        <span className="text-[10px] text-slate-400 font-sans">
          {parseInt(hour, 10) < 12 ? "上午" : parseInt(hour, 10) < 18 ? "下午" : "晚上"}
        </span>
      </div>

      {/* 现代悬浮面板 */}
      {isOpen && (
        <div className="absolute top-full left-0 mt-1.5 w-60 bg-white rounded-2xl border border-slate-200/90 shadow-2xl p-3 z-60 animate-in fade-in zoom-in-95 duration-150">
          {/* 快捷常用时段药丸 */}
          <div className="pb-2 mb-2 border-b border-slate-100">
            <span className="text-[10px] text-slate-400 font-medium block mb-1.5">快捷常用点：</span>
            <div className="flex flex-wrap gap-1">
              {quickTimes.map((t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => {
                    onChange(t);
                    setIsOpen(false);
                  }}
                  className={`px-1.5 py-0.5 rounded text-[10px] font-mono transition-all cursor-pointer border ${
                    value === t
                      ? "bg-indigo-600 text-white border-indigo-600 font-bold"
                      : "bg-slate-50 hover:bg-indigo-50 text-slate-600 hover:text-indigo-600 border-slate-200/70"
                  }`}
                >
                  {t}
                </button>
              ))}
            </div>
          </div>

          {/* 滚动选择器 (小时 : 分钟) */}
          <div className="grid grid-cols-2 gap-2 text-center">
            <div>
              <span className="text-[10px] font-bold text-slate-400 block mb-1">小时 (时)</span>
              <div className="h-40 overflow-y-auto space-y-1 pr-1 border border-slate-100 rounded-lg p-1 bg-slate-50/50">
                {hoursList.map((h) => (
                  <button
                    key={h}
                    type="button"
                    onClick={() => selectHour(h)}
                    className={`w-full py-1 rounded text-xs font-mono transition-colors cursor-pointer ${
                      hour === h
                        ? "bg-indigo-600 text-white font-bold shadow-2xs"
                        : "text-slate-700 hover:bg-slate-200/70"
                    }`}
                  >
                    {h} 时
                  </button>
                ))}
              </div>
            </div>

            <div>
              <span className="text-[10px] font-bold text-slate-400 block mb-1">分钟 (分)</span>
              <div className="h-40 overflow-y-auto space-y-1 pr-1 border border-slate-100 rounded-lg p-1 bg-slate-50/50">
                {minutesList.map((m) => (
                  <button
                    key={m}
                    type="button"
                    onClick={() => selectMinute(m)}
                    className={`w-full py-1 rounded text-xs font-mono transition-colors cursor-pointer ${
                      minute === m
                        ? "bg-indigo-600 text-white font-bold shadow-2xs"
                        : "text-slate-700 hover:bg-slate-200/70"
                    }`}
                  >
                    {m} 分
                  </button>
                ))}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
