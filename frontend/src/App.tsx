import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { marked } from "marked";
import * as XLSX from "xlsx-js-style";
import {
  Play,
  Pause,
  Square,
  Mic,
  Sparkles,
  Plus,
  Clock,
  FileText,
  Volume2,
  Trash2,
  CheckCircle2,
  Settings,
  BarChart3,
  Layers,
  Copy,
  Calendar,
  ChevronLeft,
  ChevronRight,
  Upload,
  PlusCircle,
  CalendarDays,
  List,
  StickyNote,
  CheckCheck,
  FileSpreadsheet,
  Filter,
  Download,
  Eye,
  EyeOff
} from "lucide-react";

interface Project {
  id: string;
  name: string;
  client: string;
  rate_info: string;
  timer_text: string;
  is_running: boolean;
  is_paused: boolean;
  today_duration: string;
  today_days: string;
  entries_count: number;
  currency: string;
  daily_rate: number;
  hourly_rate: number;
  billing_type: string;
  rounding_mode?: string;
  min_granularity?: number;
}

interface Entry {
  id: string;
  project_id: string;
  project_name: string;
  duration_text: string;
  days_text: string;
  amount_text: string;
  summary_text: string;
  time_range: string;
  date_str: string;
  has_audio: boolean;
  audio_path: string;
  raw_secs: number;
  is_invoiced: boolean;
}

interface DayGroup {
  date_str: string;
  display_date: string;
  entries_count: number;
  total_duration: string;
  total_days: string;
  total_amount: string;
  entries: Entry[];
}

interface GlobalStats {
  total_days: string;
  total_amount: string;
  running_count: number;
  is_recording_audio: boolean;
  active_recording_project_name: string;
  is_audio_paused: boolean;
}

interface Note {
  id: string;
  project_id: string;
  content: string;
  time_text: string;
}

export function App() {
  const [activeTab, setActiveTab] = useState<"workspace" | "stats" | "settings">("workspace");
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [dayGroups, setDayGroups] = useState<DayGroup[]>([]);
  const [notes, setNotes] = useState<Note[]>([]);
  const [globalStats, setGlobalStats] = useState<GlobalStats>({
    total_days: "0.00 人天",
    total_amount: "¥ 0.00",
    running_count: 0,
    is_recording_audio: false,
    active_recording_project_name: "",
    is_audio_paused: false,
  });

  const [noteInput, setNoteInput] = useState("");
  interface ToastInfo {
    message: string;
    actionLabel?: string;
    onAction?: () => void;
  }
  const [toast, setToast] = useState<ToastInfo | null>(null);

  const showToast = (msg: string, actionLabel?: string, onAction?: () => void) => {
    setToast({ message: msg, actionLabel, onAction });
    setTimeout(() => {
      setToast((curr) => (curr?.message === msg ? null : curr));
    }, onAction ? 8000 : 3500);
  };

  // Month & View mode
  const now = new Date();
  const currentMonthStr = `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;

  // Modals
  const [showMeetingModal, setShowMeetingModal] = useState(false);
  const [showNewProjModal, setShowNewProjModal] = useState(false);
  const [showDetailModal, setShowDetailModal] = useState(false);
  const [showManualEntryModal, setShowManualEntryModal] = useState(false);
  const [showManualMeetingModal, setShowManualMeetingModal] = useState(false);
  const [showNoteModal, setShowNoteModal] = useState(false);
  const [showFilterModal, setShowFilterModal] = useState(false);
  const [showReportExportModal, setShowReportExportModal] = useState(false);
  const [showExportSettingsModal, setShowExportSettingsModal] = useState(false);
  const [exportStrategy, setExportStrategy] = useState<"rebalance" | "min8_carry" | "cap8_carry" | "exact">("rebalance");
  const [exportSelectedProjects, setExportSelectedProjects] = useState<string[]>([]);
  const [exportSelectedMonths, setExportSelectedMonths] = useState<string[]>([currentMonthStr]);
  const [exportIncludeAmount, setExportIncludeAmount] = useState<boolean>(true); // 是否在 Excel 中导出金额
  const [hideAmount, setHideAmount] = useState<boolean>(() => {
    return localStorage.getItem("mythmanday_hide_amount") === "true" || localStorage.getItem("paraday_hide_amount") === "true";
  }); // 全局隐藏金额模式
  const [isExportingExcel, setIsExportingExcel] = useState(false);
  const [selectedEntry, setSelectedEntry] = useState<Entry | null>(null);

  const [selectedMonth, setSelectedMonth] = useState<string>(currentMonthStr);
  const [viewMode, setViewMode] = useState<"list" | "calendar">("list");
  const [calendarSelectedDate, setCalendarSelectedDate] = useState<string | null>(null);
  const [reportViewMode, setReportViewMode] = useState<"preview" | "raw">("preview");

  // Form states - New Project & 商业计费规则
  const [newProjName, setNewProjName] = useState("");
  const [newProjClient, setNewProjClient] = useState("");
  const [newProjBillingType, setNewProjBillingType] = useState<"daily" | "hourly">("daily");
  const [newProjRate, setNewProjRate] = useState("3000");
  const [newProjCurrency, setNewProjCurrency] = useState("CNY");
  const [newProjRoundingMode, setNewProjRoundingMode] = useState<"ceil" | "half_up" | "floor">("ceil");
  const [newProjMinGranularity, setNewProjMinGranularity] = useState("0.5");
  const [newProjGit, setNewProjGit] = useState("");

  // Form states - Edit Project (老项目规则配置)
  const [showEditProjModal, setShowEditProjModal] = useState(false);
  const [editProjId, setEditProjId] = useState("");
  const [editProjName, setEditProjName] = useState("");
  const [editProjClient, setEditProjClient] = useState("");
  const [editProjBillingType, setEditProjBillingType] = useState<"daily" | "hourly">("daily");
  const [editProjRate, setEditProjRate] = useState("3000");
  const [editProjCurrency, setEditProjCurrency] = useState("CNY");
  const [editProjRoundingMode, setEditProjRoundingMode] = useState<"ceil" | "half_up" | "floor">("ceil");
  const [editProjMinGranularity, setEditProjMinGranularity] = useState("0.5");
  const [editProjGit, setEditProjGit] = useState("");

  // Meeting Form (Quick Timer)
  const [meetingTopic, setMeetingTopic] = useState("");
  const [meetingRecordAudio, setMeetingRecordAudio] = useState(true);
  const [meetingSelectedPid, setMeetingSelectedPid] = useState<string>("");

  // Manual Log Entry Form
  const [manualEntryDate, setManualEntryDate] = useState(new Date().toISOString().split("T")[0]);
  const [manualEntryStartTime, setManualEntryStartTime] = useState("09:30");
  const [manualEntryEndTime, setManualEntryEndTime] = useState("12:30");
  const [manualEntrySummary, setManualEntrySummary] = useState("");

  // Manual Log Meeting Form (with Audio upload)
  const [manualMeetingDate, setManualMeetingDate] = useState(new Date().toISOString().split("T")[0]);
  const [manualMeetingStartTime, setManualMeetingStartTime] = useState("14:00");
  const [manualMeetingEndTime, setManualMeetingEndTime] = useState("15:30");
  const [manualMeetingTopic, setManualMeetingTopic] = useState("");
  const [manualMeetingRate, setManualMeetingRate] = useState("350");
  const [manualMeetingPid, setManualMeetingPid] = useState<string>("");
  const [manualMeetingAudioPath, setManualMeetingAudioPath] = useState("");

  const renderMarkdown = (content: string) => {
    if (!content) return "";
    try {
      return marked.parse(content, { breaks: true, gfm: true }) as string;
    } catch {
      return content;
    }
  };


  const refreshData = async () => {
    try {
      const projs = await invoke<Project[]>("get_projects");
      setProjects(projs);
      if (!selectedProjectId && projs.length > 0) {
        setSelectedProjectId(projs[0].id);
      }
      const stats = await invoke<GlobalStats>("get_global_stats");
      setGlobalStats(stats);
    } catch (e) {
      console.error(e);
    }
  };

  const refreshProjectDetails = async (pid: string, mStr: string = selectedMonth) => {
    if (!pid) return;
    try {
      const groups = await invoke<DayGroup[]>("get_daily_groups", {
        projectId: pid,
        month: mStr || null,
      });
      setDayGroups(groups);
      const projNotes = await invoke<Note[]>("list_project_notes", { projectId: pid });
      setNotes(projNotes);
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    refreshData();
    const interval = setInterval(() => {
      refreshData();
      if (selectedProjectId) {
        refreshProjectDetails(selectedProjectId, selectedMonth);
      }
    }, 1000);
    return () => clearInterval(interval);
  }, [selectedProjectId, selectedMonth]);

  useEffect(() => {
    if (selectedProjectId) {
      refreshProjectDetails(selectedProjectId, selectedMonth);
    }
  }, [selectedProjectId, selectedMonth]);

  const handlePrevMonth = () => {
    const [y, m] = selectedMonth.split("-").map(Number);
    const prevDate = new Date(y, m - 2, 1);
    const pStr = `${prevDate.getFullYear()}-${String(prevDate.getMonth() + 1).padStart(2, "0")}`;
    setSelectedMonth(pStr);
  };

  const handleNextMonth = () => {
    const [y, m] = selectedMonth.split("-").map(Number);
    const nextDate = new Date(y, m, 1);
    const nStr = `${nextDate.getFullYear()}-${String(nextDate.getMonth() + 1).padStart(2, "0")}`;
    setSelectedMonth(nStr);
  };

  const handleLogManualEntry = async () => {
    if (!selectedProjectId) {
      showToast("请先选择关联项目");
      return;
    }
    try {
      const res = await invoke<string>("log_manual_entry", {
        projectId: selectedProjectId,
        date: manualEntryDate,
        startTime: manualEntryStartTime,
        endTime: manualEntryEndTime,
        summary: manualEntrySummary,
      });
      setShowManualEntryModal(false);
      setManualEntrySummary("");
      showToast(res);
      refreshData();
      refreshProjectDetails(selectedProjectId, selectedMonth);
    } catch (e: any) {
      showToast("补录失败: " + e.toString());
    }
  };

  const handleLogManualMeeting = async () => {
    try {
      const res = await invoke<string>("log_manual_meeting", {
        projectId: manualMeetingPid || null,
        date: manualMeetingDate,
        startTime: manualMeetingStartTime,
        endTime: manualMeetingEndTime,
        topic: manualMeetingTopic || "技术方案沟通会",
        rate: parseFloat(manualMeetingRate) || 350,
        sourceAudioPath: manualMeetingAudioPath.trim() || null,
      });
      setShowManualMeetingModal(false);
      setManualMeetingTopic("");
      setManualMeetingAudioPath("");
      showToast(res);
      refreshData();
      if (selectedProjectId) refreshProjectDetails(selectedProjectId, selectedMonth);
    } catch (e: any) {
      showToast("补录失败: " + e.toString());
    }
  };

  const activeProject = projects.find((p) => p.id === selectedProjectId);

  const handleStart = async (id: string) => {
    await invoke("start_timer", { projectId: id });
    showToast("▶ 工时通道已启动计时");
    refreshData();
  };

  const handlePause = async (id: string) => {
    await invoke("pause_timer", { projectId: id });
    showToast("⏸ 工时已挂起暂停");
    refreshData();
  };

  const handleResume = async (id: string) => {
    await invoke("resume_timer", { projectId: id });
    showToast("▶ 已继续工时计时");
    refreshData();
  };

  const handleStop = async (id: string) => {
    const res = await invoke<string>("stop_timer", { projectId: id });
    showToast(res);
    refreshData();
    if (selectedProjectId) refreshProjectDetails(selectedProjectId);
  };

  const handleAddNote = async () => {
    if (!noteInput.trim() || !selectedProjectId) return;
    await invoke("add_project_note", { projectId: selectedProjectId, content: noteInput });
    setNoteInput("");
    showToast("备忘速记已存入");
    refreshProjectDetails(selectedProjectId);
  };

  const handleDeleteNote = async (nid: string) => {
    await invoke("delete_note", { noteId: nid });
    showToast("备忘已删除");
    if (selectedProjectId) refreshProjectDetails(selectedProjectId);
  };

  const handleCreateProject = async () => {
    if (!newProjName.trim()) {
      showToast("项目名称不能为空");
      return;
    }
    try {
      const parsedRate = parseFloat(newProjRate) || 0;
      const parsedGran = parseFloat(newProjMinGranularity) || 0.5;

      const pid = await invoke<string>("create_project", {
        name: newProjName,
        client: newProjClient,
        billingType: newProjBillingType,
        rate: parsedRate,
        dailyRate: newProjBillingType === "daily" ? parsedRate : parsedRate * 8.0,
        currency: newProjCurrency.trim().toUpperCase() || "CNY",
        roundingMode: newProjRoundingMode,
        minGranularity: parsedGran,
        gitRepo: newProjGit || null,
      });
      setShowNewProjModal(false);
      setNewProjName("");
      setNewProjClient("");
      setNewProjRate(newProjBillingType === "daily" ? "3000" : "400");
      showToast("🎉 新商业项目与计费规则创建成功");
      setSelectedProjectId(pid);
      refreshData();
    } catch (e: any) {
      showToast(e.toString());
    }
  };

  const handleOpenEditProject = (proj: Project) => {
    setEditProjId(proj.id);
    setEditProjName(proj.name);
    setEditProjClient(proj.client || "");
    const isHourly = proj.billing_type === "hourly";
    setEditProjBillingType(isHourly ? "hourly" : "daily");
    setEditProjRate(isHourly ? (proj.hourly_rate ? proj.hourly_rate.toString() : "400") : (proj.daily_rate ? proj.daily_rate.toString() : "3000"));
    setEditProjCurrency(proj.currency || "CNY");
    setEditProjRoundingMode((proj.rounding_mode as any) || "ceil");
    setEditProjMinGranularity(proj.min_granularity !== undefined ? proj.min_granularity.toString() : "0.5");
    setEditProjGit("");
    setShowEditProjModal(true);
  };

  const handleUpdateProject = async () => {
    if (!editProjId) return;
    if (!editProjName.trim()) {
      showToast("项目名称不能为空");
      return;
    }
    try {
      const parsedRate = parseFloat(editProjRate) || 0;
      const parsedGran = parseFloat(editProjMinGranularity) || 0.5;

      await invoke("update_project", {
        id: editProjId,
        name: editProjName,
        client: editProjClient,
        billingType: editProjBillingType,
        rate: parsedRate,
        currency: editProjCurrency.trim().toUpperCase() || "CNY",
        roundingMode: editProjRoundingMode,
        minGranularity: parsedGran,
        gitRepo: editProjGit ? editProjGit.trim() : null,
      });

      setShowEditProjModal(false);
      showToast("✅ 项目计费规则已更新并保存");
      await refreshData();
      if (selectedProjectId) {
        refreshProjectDetails(selectedProjectId);
      }
    } catch (e: any) {
      showToast("更新失败: " + e.toString());
    }
  };

  const handleStartMeeting = async () => {
    try {
      const res = await invoke<string>("start_quick_meeting", {
        projectId: meetingSelectedPid || null,
        topic: meetingTopic.trim() || "技术方案沟通评审",
        recordAudio: meetingRecordAudio,
      });
      setShowMeetingModal(false);
      setMeetingTopic("");
      showToast(res);
      refreshData();
    } catch (e: any) {
      showToast(e.toString());
    }
  };

  const handleTranscribe = async (entry: Entry) => {
    showToast("🤖 AI 正在离线/在线识别音频并总结纪要...");
    try {
      await invoke<string>("transcribe_entry_audio", {
        entryId: entry.id,
        audioPath: entry.audio_path,
      });
      showToast("AI 会议纪要提炼完成！");
      if (selectedProjectId) refreshProjectDetails(selectedProjectId);
    } catch (e: any) {
      showToast("转写失败: " + e.toString());
    }
  };

  // 结算状态与报表多维导出状态
  interface SettlementReportData {
    markdown: string;
    total_days: number;
    total_amount: number;
    settled_amount: number;
    pending_amount: number;
    matching_entry_ids: string[];
    entries: Entry[];
  }

  const [reportSelectedProjects, setReportSelectedProjects] = useState<string[]>([]);
  const [reportSelectedMonths, setReportSelectedMonths] = useState<string[]>([currentMonthStr]);
  const [reportStatusFilter, setReportStatusFilter] = useState<"all" | "pending" | "settled">("all");
  const [currentStrategy, setCurrentStrategy] = useState<"rebalance" | "min8_carry" | "cap8_carry" | "exact">("rebalance");
  const [settlementReport, setSettlementReport] = useState<SettlementReportData | null>(null);

  const handleToggleEntryInvoiced = async (entryId: string, currentStatus: boolean) => {
    try {
      await invoke("toggle_entry_invoiced", {
        entryId,
        isInvoiced: !currentStatus,
      });
      showToast(!currentStatus ? "✅ 已标记为已结算！" : "⏳ 已重置为待结算状态");
      if (selectedProjectId) refreshProjectDetails(selectedProjectId);
      if (activeTab === "stats") {
        fetchSettlementReport();
      }
    } catch (e: any) {
      showToast("操作失败: " + e.toString());
    }
  };

  const handleBatchToggleInvoiced = async (isInvoiced: boolean) => {
    if (!settlementReport || settlementReport.matching_entry_ids.length === 0) {
      showToast("当前无匹配条目可批量操作");
      return;
    }
    try {
      const count = await invoke<number>("batch_toggle_entries_invoiced", {
        entryIds: settlementReport.matching_entry_ids,
        isInvoiced,
      });
      showToast(`已将匹配的 ${count} 笔工时流水标记为「${isInvoiced ? "已结算" : "待结算"}」！`);
      if (selectedProjectId) refreshProjectDetails(selectedProjectId);
      fetchSettlementReport();
    } catch (e: any) {
      showToast("批量操作失败: " + e.toString());
    }
  };

  const fetchSettlementReport = async (overrideStrategy?: "rebalance" | "min8_carry" | "cap8_carry" | "exact") => {
    const strat = overrideStrategy || currentStrategy;
    try {
      const res = await invoke<SettlementReportData>("generate_settlement_report", {
        projectIds: reportSelectedProjects.length > 0 ? reportSelectedProjects : null,
        months: reportSelectedMonths.length > 0 ? reportSelectedMonths : null,
        statusFilter: reportStatusFilter,
        enableRebalance: strat === "rebalance",
        settlementStrategy: strat,
      });
      setSettlementReport(res);
    } catch (e: any) {
      showToast("生成结算报表失败: " + e.toString());
    }
  };

  // 格式化工作交付条目为规范换行编号
  const formatDeliverablesToNumberedLines = (rawSummary: string): string => {
    if (!rawSummary) return "持续代码开发与测试交付";
    const clean = rawSummary
      .replace(/\[削峰回填\]\s*/g, "")
      .replace(/\[超额顺延\]\s*/g, "");
    const parts = clean
      .split(/[；;。\n]+/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);
    if (parts.length === 0) return "持续代码开发与测试交付";
    if (parts.length === 1) return parts[0];
    return parts.map((p, idx) => `${idx + 1}. ${p}`).join("\n");
  };

  // 格式化投入工时：以小时为单位，最小刻度为0.5，不足0.5按0.5，超过按1 (纯数字，单位在表头注明)
  const formatToHalfHours = (rawText: string): string => {
    if (!rawText) return "0.0";
    // 提取数字并转换
    const cleanStr = rawText.replace(/小时/g, "").trim();
    if (cleanStr.includes(":")) {
      const parts = cleanStr.split(":").map((v) => parseInt(v, 10) || 0);
      let totalHours = 0;
      if (parts.length >= 2) {
        totalHours = parts[0] + parts[1] / 60 + (parts[2] || 0) / 3600;
      } else {
        totalHours = parseFloat(cleanStr) || 0;
      }
      if (totalHours <= 0) return "0.0";
      const stepped = Math.ceil(totalHours * 2) / 2;
      return stepped.toFixed(1);
    }

    const val = parseFloat(cleanStr);
    if (!isNaN(val) && val > 0) {
      const stepped = Math.ceil(val * 2) / 2;
      return stepped.toFixed(1);
    }
    return "0.0";
  };

  // 提取纯数字金额与天数字符串
  const formatPureNumber = (rawText: string): string => {
    if (!rawText) return "0.00";
    const match = rawText.match(/([\d.]+)/);
    if (match) {
      const v = parseFloat(match[1]);
      return isNaN(v) ? "0.00" : v.toFixed(2);
    }
    return "0.00";
  };

  // 核心多项目·多月份按Sheet专业 Excel 导出执行
  const executeExportExcel = async () => {
    const targetProjectIds = exportSelectedProjects.length > 0
      ? exportSelectedProjects
      : projects.map((p) => p.id);

    if (targetProjectIds.length === 0) {
      showToast("请先选择至少一个要导出的项目！");
      return;
    }

    const targetMonths = exportSelectedMonths.length > 0
      ? exportSelectedMonths
      : [currentMonthStr];

    setIsExportingExcel(true);
    setShowExportSettingsModal(false);
    showToast("📊 正在按项目生成独立对账单文件...");

    try {
      const dateTag = new Date().toISOString().split("T")[0];
      const filesPayload: { filename: string; base64_data: string }[] = [];

      // 遍历每个被选中的项目，单独生成一个专属的 Workbook
      for (const pid of targetProjectIds) {
        const proj = projects.find((p) => p.id === pid);
        if (!proj) continue;

        const workbook = XLSX.utils.book_new();

        // 针对该项目的各个选择月份，每个月生成一个单独的 Sheet
        for (const monthStr of targetMonths) {
          // 调用后端带策略引擎的数据接口
          const reportRes = await invoke<SettlementReportData>("generate_settlement_report", {
            projectIds: [pid],
            months: [monthStr],
            statusFilter: reportStatusFilter,
            enableRebalance: exportStrategy === "rebalance",
            settlementStrategy: exportStrategy,
          });

          // 如果该月没有记录，仍然生成一个清晰的标准空模板 Sheet
          const entries = reportRes.entries || [];

          // 1. 构建商务抬头部信息
          const strategyLabels: Record<string, string> = {
            rebalance: "合规削峰填谷 (单日8h封顶·工作日平摊)",
            min8_carry: "保底8小时模式 (不足8h按8h/天·超额顺延新天)",
            cap8_carry: "封顶顺延模式 (单日8h封顶·超额顺延新天)",
            exact: "实际物理工时模式 (实报实销·同日合并)",
          };

          // 1. 构建商务抬头部信息 (规范 2 列键值对排版，避免挤压串列)
          const colCount = exportIncludeAmount ? 8 : 7;
          const headerColumns = exportIncludeAmount
            ? ["序号", "核算日期", "时间范围", "投入工时 (小时)", "折算人天 (天)", `核算金额 (${proj.currency})`, "结算状态", "工作交付清单与产出成果"]
            : ["序号", "核算日期", "时间范围", "投入工时 (小时)", "折算人天 (天)", "结算状态", "工作交付清单与产出成果"];

          const sheetHeaderData = [
            ["【项目人天结算对账单】", ...Array(colCount - 1).fill("")],
            ["项目名称:", proj.name, "", "客户 / 委托方:", proj.client || "商业客户", "", "核算月份:", monthStr],
            ["结算标准:", exportIncludeAmount ? `${proj.daily_rate} ${proj.currency}/天 (8.0h/天)` : "8.0h/天 标准人天", "", "核算策略:", strategyLabels[exportStrategy], "", "结算状态:", reportStatusFilter === "all" ? "全部工时" : reportStatusFilter === "settled" ? "已结算" : "待结算"],
            ["导出日期:", dateTag, ...Array(colCount - 2).fill("")],
            [], // 空行分隔
            headerColumns,
          ];

          // 2. 排序并填充明细流水行 (正序升序：从月初到月末按时间先后排序，纯数字无冗余单位)
          const sortedEntries = [...entries].sort((a, b) => a.date_str.localeCompare(b.date_str));
          const sheetRows: any[][] = [];
          sortedEntries.forEach((item, idx) => {
            if (exportIncludeAmount) {
              sheetRows.push([
                idx + 1,
                item.date_str,
                item.time_range,
                parseFloat(formatToHalfHours(item.duration_text)),
                parseFloat(formatPureNumber(item.days_text)),
                parseFloat(formatPureNumber(item.amount_text)),
                item.is_invoiced ? "已结算" : "待结算",
                formatDeliverablesToNumberedLines(item.summary_text),
              ]);
            } else {
              sheetRows.push([
                idx + 1,
                item.date_str,
                item.time_range,
                parseFloat(formatToHalfHours(item.duration_text)),
                parseFloat(formatPureNumber(item.days_text)),
                item.is_invoiced ? "已结算" : "待结算",
                formatDeliverablesToNumberedLines(item.summary_text),
              ]);
            }
          });

          // 3. 构建汇总行
          const totalDays = sortedEntries.reduce((acc, curr) => {
            const v = parseFloat(curr.days_text.replace(/[^\d.]/g, "")) || 0;
            return acc + v;
          }, 0);

          const totalAmount = sortedEntries.reduce((acc, curr) => {
            const v = parseFloat(curr.amount_text.replace(/[^\d.]/g, "")) || 0;
            return acc + v;
          }, 0);

          sheetRows.push([]);
          if (exportIncludeAmount) {
            sheetRows.push([
              "【月度汇总】",
              `共计 ${sortedEntries.length} 个工作日`,
              "",
              "",
              parseFloat(totalDays.toFixed(2)),
              parseFloat(totalAmount.toFixed(2)),
              "",
              `核算规则: ${strategyLabels[exportStrategy]}`,
            ]);
          } else {
            sheetRows.push([
              "【月度汇总】",
              `共计 ${sortedEntries.length} 个工作日`,
              "",
              "",
              parseFloat(totalDays.toFixed(2)),
              "",
              `核算规则: ${strategyLabels[exportStrategy]}`,
            ]);
          }

          // 4. 将二维数据矩阵转为 Sheet
          const worksheet = XLSX.utils.aoa_to_sheet([...sheetHeaderData, ...sheetRows]);

          // 设置单元格合并 (Merges): 大标题跨列合并 A1:H1 或 A1:G1
          worksheet["!merges"] = [
            { s: { r: 0, c: 0 }, e: { r: 0, c: colCount - 1 } }, // 大标题合并
            { s: { r: 1, c: 1 }, e: { r: 1, c: 2 } }, // 项目名称跨2列
            { s: { r: 1, c: 4 }, e: { r: 1, c: 5 } }, // 客户跨2列
            { s: { r: 2, c: 1 }, e: { r: 2, c: 2 } }, // 结算标准跨2列
            { s: { r: 2, c: 4 }, e: { r: 2, c: 5 } }, // 核算策略跨2列
          ];

          // 5. 应用单元格样式与自动换行 (wrapText: true)
          // 遍历表头和数据行设置专业字体与对齐方式
          // 标题行样式 (Row 0: A1)
          const titleCell = worksheet["A1"];
          if (titleCell) {
            titleCell.s = {
              font: { name: "微软雅黑", sz: 16, bold: true, color: { rgb: "1E293B" } },
              alignment: { horizontal: "center", vertical: "center" },
            };
          }

          // 表头列名称 (Row 5: A6:H6 或 A6:G6)
          const colLetters = exportIncludeAmount
            ? ["A", "B", "C", "D", "E", "F", "G", "H"]
            : ["A", "B", "C", "D", "E", "F", "G"];
          const lastColLetter = colLetters[colLetters.length - 1];

          for (const col of colLetters) {
            const ref = `${col}6`;
            if (worksheet[ref]) {
              worksheet[ref].s = {
                font: { name: "微软雅黑", sz: 10, bold: true, color: { rgb: "334155" } },
                fill: { fgColor: { rgb: "F1F5F9" } },
                alignment: {
                  horizontal: col === lastColLetter ? "left" : "center",
                  vertical: "center",
                },
                border: {
                  top: { style: "thin", color: { rgb: "CBD5E1" } },
                  bottom: { style: "medium", color: { rgb: "94A3B8" } },
                },
              };
            }
          }

          // 数据行格式化 (从第7行开始，即 r = 6)
          const dataStartRow = 6;
          for (let i = 0; i < sortedEntries.length; i++) {
            const r = dataStartRow + i;

            for (let c = 0; c < colCount; c++) {
              const cellRef = XLSX.utils.encode_cell({ r, c });
              const cell = worksheet[cellRef];
              if (!cell) continue;

              if (c === colCount - 1) {
                // 最后一列: 工作交付清单与产出成果，设置自动换行和靠左居上
                cell.s = {
                  font: { name: "微软雅黑", sz: 10, color: { rgb: "1E293B" } },
                  alignment: {
                    wrapText: true,
                    vertical: "top",
                    horizontal: "left",
                  },
                  border: {
                    bottom: { style: "thin", color: { rgb: "E2E8F0" } },
                  },
                };
              } else {
                // 数字列 (D:投入工时, E:折算人天, [F:核算金额])
                let numFmt = undefined;
                if (c === 3) numFmt = "0.0";
                else if (c === 4) numFmt = "0.00";
                else if (exportIncludeAmount && c === 5) numFmt = "#,##0.00";

                const isStatusCol = exportIncludeAmount ? c === 6 : c === 5;

                cell.s = {
                  font: { name: "微软雅黑", sz: 10, color: { rgb: "334155" } },
                  alignment: {
                    vertical: "top",
                    horizontal: c === 0 || c === 1 || c === 2 || isStatusCol ? "center" : "right",
                  },
                  border: {
                    bottom: { style: "thin", color: { rgb: "E2E8F0" } },
                  },
                  ...(numFmt ? { numFmt } : {}),
                };
              }
            }
          }

          // 汇总行样式 (最后两行，即倒数第一行)
          const summaryRowIdx = dataStartRow + sortedEntries.length + 1;
          for (let c = 0; c < colCount; c++) {
            const cellRef = XLSX.utils.encode_cell({ r: summaryRowIdx, c });
            const cell = worksheet[cellRef];
            if (cell) {
              let numFmt = undefined;
              if (c === 4) numFmt = "0.00";
              else if (exportIncludeAmount && c === 5) numFmt = "#,##0.00";

              cell.s = {
                font: { name: "微软雅黑", sz: 10, bold: true, color: { rgb: "0F172A" } },
                fill: { fgColor: { rgb: "F8FAFC" } },
                alignment: {
                  vertical: "center",
                  horizontal: c === 4 || (exportIncludeAmount && c === 5) ? "right" : "left",
                },
                border: {
                  top: { style: "thin", color: { rgb: "CBD5E1" } },
                  bottom: { style: "double", color: { rgb: "64748B" } },
                },
                ...(numFmt ? { numFmt } : {}),
              };
            }
          }

          // 设置专业列宽 (自适应宽度，避免截断和挤压)
          worksheet["!cols"] = exportIncludeAmount
            ? [
                { wch: 10 },  // A: 序号
                { wch: 15 },  // B: 核算日期
                { wch: 15 },  // C: 时间范围
                { wch: 14 },  // D: 投入工时
                { wch: 14 },  // E: 折算人天
                { wch: 16 },  // F: 核算金额
                { wch: 12 },  // G: 结算状态
                { wch: 70 },  // H: 工作交付清单与产出成果
              ]
            : [
                { wch: 10 },  // A: 序号
                { wch: 15 },  // B: 核算日期
                { wch: 15 },  // C: 时间范围
                { wch: 14 },  // D: 投入工时
                { wch: 14 },  // E: 折算人天
                { wch: 12 },  // F: 结算状态
                { wch: 70 },  // G: 工作交付清单与产出成果
              ];

          // Sheet 命名为 "YYYY年MM月"
          const [y, m] = monthStr.split("-");
          const sheetName = `${y}年${m}月`;
          XLSX.utils.book_append_sheet(workbook, worksheet, sheetName);
        }

        // 文件命名规则：《项目名_人天结算对账单_导出日期.xlsx》
        const safeProjName = proj.name.replace(/[\\/:*?"<>|]/g, "_");
        const fileName = `${safeProjName}_人天结算对账单_${dateTag}.xlsx`;
        const b64Data = XLSX.write(workbook, { bookType: "xlsx", type: "base64" });

        filesPayload.push({
          filename: fileName,
          base64_data: b64Data,
        });
      }

      if (filesPayload.length === 0) {
        showToast("没有符合条件的对账单可供导出！");
        return;
      }

      // 保存全部文件并打开 Downloads 目录
      const savedPaths = await invoke<string[]>("save_multiple_excel_files", {
        files: filesPayload,
      });

      showToast(
        `🎉 成功导出 ${savedPaths.length} 个项目独立对账单 (每个月独立Sheet)！`,
        "📂 查看 Downloads 文件夹",
        () => {
          if (savedPaths.length > 0) {
            invoke("open_file_or_dir", { path: savedPaths[0] });
          }
        }
      );
    } catch (err: any) {
      showToast("导出失败: " + err.toString());
    } finally {
      setIsExportingExcel(false);
    }
  };

  return (
    <div className="flex h-screen w-screen bg-[#f1f5f9] text-slate-800 font-sans select-none antialiased overflow-hidden">
      {/* ---------------- 现代左侧导航侧边栏 (极简高级深灰调) ---------------- */}
      <aside className="w-60 bg-[#0f172a] text-slate-300 flex flex-col justify-between p-4 shrink-0 shadow-xl border-r border-slate-800/80">
        <div className="space-y-6">
          {/* 品牌标识 */}
          <div className="flex items-center gap-3 px-1 pt-1">
            <img 
              src="/logo.png" 
              alt="Logo" 
              className="w-8 h-8 rounded-lg shadow-sm shadow-indigo-500/30 object-contain bg-slate-900" 
            />
            <div>
              <h1 className="font-extrabold text-sm text-white tracking-wide">人天神话</h1>
              <p className="text-[10px] font-medium text-indigo-300/80 tracking-tight">The Mythical Man-Day</p>
            </div>
          </div>

          {/* 快捷主操作：发起会议沟通 */}
          <button
            onClick={() => setShowMeetingModal(true)}
            className="w-full h-9 px-3 bg-indigo-600 hover:bg-indigo-500 active:scale-[0.98] transition-all text-white font-medium text-xs rounded-lg flex items-center justify-center gap-2 shadow-sm shadow-indigo-600/30"
          >
            <Mic className="w-3.5 h-3.5" />
            发起会议沟通
          </button>

          {/* 导航菜单 */}
          <nav className="space-y-1">
            <div className="px-2.5 pb-1 text-[10px] font-semibold text-slate-500 uppercase tracking-wider">工作台</div>
            <button
              onClick={() => setActiveTab("workspace")}
              className={`w-full h-9 px-2.5 rounded-lg flex items-center justify-between text-xs font-medium transition-colors ${
                activeTab === "workspace"
                  ? "bg-slate-800 text-white shadow-xs"
                  : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200"
              }`}
            >
              <div className="flex items-center gap-2.5">
                <Layers className={`w-4 h-4 ${activeTab === "workspace" ? "text-indigo-400" : "text-slate-400"}`} />
                项目工作区
              </div>
              {globalStats.running_count > 0 && (
                <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
              )}
            </button>

            <button
              onClick={() => {
                setActiveTab("stats");
                fetchSettlementReport();
              }}
              className={`w-full h-9 px-2.5 rounded-lg flex items-center justify-between text-xs font-medium transition-colors ${
                activeTab === "stats"
                  ? "bg-slate-800 text-white shadow-xs"
                  : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200"
              }`}
            >
              <div className="flex items-center gap-2.5">
                <BarChart3 className={`w-4 h-4 ${activeTab === "stats" ? "text-indigo-400" : "text-slate-400"}`} />
                全局工时统计
              </div>
            </button>

            <button
              onClick={() => setActiveTab("settings")}
              className={`w-full h-9 px-2.5 rounded-lg flex items-center gap-2.5 text-xs font-medium transition-colors ${
                activeTab === "settings"
                  ? "bg-slate-800 text-white shadow-xs"
                  : "text-slate-400 hover:bg-slate-800/50 hover:text-slate-200"
              }`}
            >
              <Settings className={`w-4 h-4 ${activeTab === "settings" ? "text-indigo-400" : "text-slate-400"}`} />
              系统与AI引擎
            </button>
          </nav>
        </div>

        {/* 侧边栏底部累计投入指标 (选项 2: 仅专注人天与工时投入，隐去核算金额) */}
        <div className="p-3 bg-slate-800/80 rounded-xl border border-slate-700/60 shadow-inner space-y-1">
          <div className="flex items-center justify-between text-[11px] font-medium text-slate-400">
            <span>今日累计投入</span>
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-slate-700/70 text-indigo-300 font-mono">
              工时核算
            </span>
          </div>
          <div className="text-xl font-bold text-white tracking-tight">{globalStats.total_days}</div>
          <div className="text-[11px] text-slate-400 flex items-center justify-between pt-1 border-t border-slate-700/60">
            <span>运行通道</span>
            <span className="font-medium text-slate-300">
              {globalStats.running_count > 0 ? (
                <span className="text-emerald-400 font-semibold">{globalStats.running_count} 个进行中</span>
              ) : (
                "暂无进行中"
              )}
            </span>
          </div>
        </div>
      </aside>

      {/* ---------------- 主内容展示画布 ---------------- */}
      <main className="flex-1 flex flex-col min-w-0 bg-[#f1f5f9] overflow-hidden">
        {/* 顶部全局顶栏 */}
        <header className="h-14 border-b border-slate-200/90 bg-white px-6 flex items-center justify-between shrink-0 shadow-2xs">
          <div>
            <h2 className="text-sm font-bold text-slate-900">
              {activeTab === "workspace" && "项目工时工作区"}
              {activeTab === "stats" && "全局工时透视与技术日报"}
              {activeTab === "settings" && "系统设置与 AI 引擎配置"}
            </h2>
            <p className="text-[11px] text-slate-500">
              开发者人天中台 · 多通道并发工时 · 录音与交付凭证一体化沉淀
            </p>
          </div>

          <div className="flex items-center gap-3">
            {/* 活动通道徽章 */}
            <div
              className={`h-7 px-3 rounded-full border flex items-center gap-1.5 text-xs font-medium ${
                globalStats.running_count > 0
                  ? "bg-emerald-50 border-emerald-200 text-emerald-700"
                  : "bg-slate-100 border-slate-200 text-slate-600"
              }`}
            >
              <span
                className={`w-1.5 h-1.5 rounded-full ${
                  globalStats.running_count > 0 ? "bg-emerald-500 animate-pulse" : "bg-slate-400"
                }`}
              />
              {globalStats.running_count > 0 ? `${globalStats.running_count} 个通道计时中` : "无活动通道"}
            </div>

            {/* 全局金额隐私模式切换 */}
            <button
              onClick={() => {
                const nextVal = !hideAmount;
                setHideAmount(nextVal);
                localStorage.setItem("mythmanday_hide_amount", String(nextVal));
                showToast(nextVal ? "🔒 已开启隐私模式：全局隐藏金额" : "👁️ 已关闭隐私模式：正常显示金额");
              }}
              className={`h-8 px-2.5 rounded-lg border text-xs font-semibold flex items-center gap-1.5 transition-all shadow-xs cursor-pointer ${
                hideAmount
                  ? "bg-amber-50 border-amber-300 text-amber-800 hover:bg-amber-100"
                  : "bg-white border-slate-200 text-slate-700 hover:bg-slate-50"
              }`}
              title={hideAmount ? "当前已隐藏金额，点击恢复显示" : "点击开启隐私模式，全局隐藏金额"}
            >
              {hideAmount ? <EyeOff className="w-3.5 h-3.5 text-amber-600" /> : <Eye className="w-3.5 h-3.5 text-slate-500" />}
              <span>{hideAmount ? "金额已隐藏" : "显示金额"}</span>
            </button>

            <button
              onClick={() => setShowNewProjModal(true)}
              className="h-8 px-3 rounded-lg border border-slate-200 bg-white hover:bg-slate-50 text-slate-700 text-xs font-semibold flex items-center gap-1.5 shadow-xs transition-all cursor-pointer"
            >
              <Plus className="w-3.5 h-3.5 text-slate-500" />
              新建项目
            </button>
          </div>
        </header>

        {/* 录音微幅浮动横幅 */}
        {globalStats.is_recording_audio && (
          <div className="bg-amber-50 border-b border-amber-200 px-6 py-2 flex items-center justify-between text-xs font-medium text-amber-900">
            <div className="flex items-center gap-2">
              <span className="w-2 h-2 rounded-full bg-amber-600 animate-ping" />
              <span>
                🎙️ 全程会议录音中: <strong className="font-bold">{globalStats.active_recording_project_name}</strong>
              </span>
            </div>
            <div className="flex items-center gap-2">
              <button
                onClick={() => invoke("toggle_audio_pause")}
                className="px-2.5 py-1 bg-amber-600 hover:bg-amber-700 text-white rounded text-[11px] font-semibold"
              >
                {globalStats.is_audio_paused ? "▶ 继续录音" : "⏸ 暂停录音"}
              </button>
            </div>
          </div>
        )}

        {/* 选项卡视图内容 */}
        {activeTab === "workspace" && (
          <div className="flex-1 flex min-h-0 p-5 gap-5 overflow-hidden">
            {/* 左侧：项目卡片侧栏列表 */}
            <div className="w-72 flex flex-col bg-white border border-slate-200/90 rounded-xl shadow-xs shrink-0 overflow-hidden">
              <div className="p-3.5 border-b border-slate-100 flex items-center justify-between bg-slate-50/50">
                <span className="text-xs font-bold text-slate-700 flex items-center gap-1.5">
                  <Layers className="w-3.5 h-3.5 text-slate-400" />
                  研发与会议项目
                </span>
                <span className="text-[11px] font-medium text-slate-400">{projects.length} 个项目</span>
              </div>

              <div className="flex-1 overflow-y-auto p-2.5 space-y-1.5">
                {projects.map((p) => {
                  const isSelected = selectedProjectId === p.id;
                  return (
                    <div
                      key={p.id}
                      onClick={() => setSelectedProjectId(p.id)}
                      className={`p-3 rounded-lg border transition-all cursor-pointer ${
                        isSelected
                          ? "bg-slate-900 border-slate-900 text-white shadow-xs"
                          : "bg-white hover:bg-slate-50 border-slate-100 text-slate-700"
                      }`}
                    >
                      <div className="flex items-center justify-between mb-1">
                        <div className="flex items-center gap-2 min-w-0">
                          <span
                            className={`w-2 h-2 rounded-full shrink-0 ${
                              p.is_running
                                ? "bg-emerald-400 animate-pulse"
                                : p.is_paused
                                ? "bg-amber-400"
                                : isSelected
                                ? "bg-slate-600"
                                : "bg-slate-300"
                            }`}
                          />
                          <span className={`text-xs font-semibold truncate ${isSelected ? "text-white" : "text-slate-800"}`}>
                            {p.name}
                          </span>
                        </div>
                        {p.is_running && (
                          <span className="px-1.5 py-0.2 rounded text-[10px] font-medium bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                            计时中
                          </span>
                        )}
                      </div>

                      <div className="flex items-center justify-between text-[11px] opacity-80">
                        <span className="truncate max-w-[130px]">{p.client} · {p.entries_count}条</span>
                        <span className={`font-medium ${isSelected ? "text-indigo-300" : "text-indigo-600"}`}>
                          {p.today_duration !== "00:00:00" ? p.today_duration : ""}
                        </span>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            {/* 右侧：选中项目的工时明细看板 (一天一个卡片) */}
            <div className="flex-1 flex flex-col min-w-0 overflow-y-auto pr-1 space-y-4">
              {activeProject && (
                <>
                  {/* 项目主卡片 Header */}
                  <div className="bg-white border border-slate-200/80 rounded-xl p-5 shadow-sm space-y-3.5">
                    <div className="flex items-center justify-between gap-4">
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2.5 flex-wrap">
                          <h3 className="text-base font-bold text-slate-900 tracking-tight truncate">
                            {activeProject.name}
                          </h3>
                          <span className="px-2 py-0.5 rounded bg-slate-100 text-slate-600 text-xs font-medium whitespace-nowrap">
                            {activeProject.client}
                          </span>
                          <button
                            onClick={() => handleOpenEditProject(activeProject)}
                            className="px-2 py-0.5 rounded bg-indigo-50 hover:bg-indigo-100 border border-indigo-200/80 text-indigo-700 text-xs font-medium flex items-center gap-1 transition-all cursor-pointer shadow-2xs"
                            title="配置并修改项目的商业计费方式、价格单价、币种与舍入规则"
                          >
                            <Settings className="w-3 h-3 text-indigo-600" />
                            规则配置
                          </button>
                          {activeProject.is_running && (
                            <span className="px-2 py-0.5 rounded bg-emerald-50 text-emerald-700 text-xs font-semibold flex items-center gap-1.5 whitespace-nowrap">
                              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-ping" />
                              正在计时
                            </span>
                          )}
                        </div>
                        <div className="flex items-center gap-2 text-xs text-slate-500 mt-1.5 flex-wrap">
                          <span className="whitespace-nowrap">
                            标准: {hideAmount ? "****** (金额已隐藏)" : activeProject.rate_info}
                          </span>
                          <span className="text-slate-300">•</span>
                          <span className="whitespace-nowrap">
                            今日投入: <strong className="text-slate-800 font-semibold">{activeProject.today_duration}</strong> ({activeProject.today_days})
                          </span>
                        </div>
                      </div>

                      {/* 大时钟与控制按钮 */}
                      <div className="flex items-center gap-4 shrink-0">
                        <div
                          className={`text-2xl font-bold font-mono tracking-tight ${
                            activeProject.is_running ? "text-emerald-600" : "text-slate-700"
                          }`}
                        >
                          {activeProject.timer_text}
                        </div>

                        {!activeProject.is_running && !activeProject.is_paused && (
                          <div className="flex items-center gap-2">
                            <button
                              onClick={() => handleStart(activeProject.id)}
                              className="h-8 px-3.5 bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs active:scale-95 transition-all"
                            >
                              <Play className="w-3.5 h-3.5 fill-current" />
                              开始工时
                            </button>
                            <button
                              onClick={() => {
                                setMeetingSelectedPid(activeProject.id);
                                setShowMeetingModal(true);
                              }}
                              className="h-8 px-3 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs active:scale-95 transition-all"
                            >
                              <Mic className="w-3.5 h-3.5" />
                              发起会议
                            </button>
                            <button
                              onClick={() => setShowNoteModal(true)}
                              className="h-8 px-3 bg-slate-100 hover:bg-slate-200 border border-slate-200/80 text-slate-700 rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-2xs active:scale-95 transition-all"
                              title="记录工作便签与备忘"
                            >
                              <StickyNote className="w-3.5 h-3.5 text-amber-500" />
                              便签
                            </button>
                          </div>
                        )}

                        {activeProject.is_running && (
                          <div className="flex items-center gap-2">
                            <button
                              onClick={() => setShowNoteModal(true)}
                              className="h-8 px-2.5 bg-slate-100 hover:bg-slate-200 border border-slate-200 text-slate-700 rounded-lg text-xs font-medium flex items-center gap-1 shadow-2xs transition-all"
                            >
                              <StickyNote className="w-3.5 h-3.5 text-amber-500" />
                              便签
                            </button>
                            <button
                              onClick={() => handlePause(activeProject.id)}
                              className="h-8 px-3 bg-amber-500 hover:bg-amber-600 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs transition-all"
                            >
                              <Pause className="w-3.5 h-3.5 fill-current" />
                              暂停
                            </button>
                            <button
                              onClick={() => handleStop(activeProject.id)}
                              className="h-8 px-3 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs transition-all"
                            >
                              <Square className="w-3.5 h-3.5 fill-current" />
                              结束工时
                            </button>
                          </div>
                        )}

                        {activeProject.is_paused && (
                          <div className="flex items-center gap-2">
                            <button
                              onClick={() => setShowNoteModal(true)}
                              className="h-8 px-2.5 bg-slate-100 hover:bg-slate-200 border border-slate-200 text-slate-700 rounded-lg text-xs font-medium flex items-center gap-1 shadow-2xs transition-all"
                            >
                              <StickyNote className="w-3.5 h-3.5 text-amber-500" />
                              便签
                            </button>
                            <button
                              onClick={() => handleResume(activeProject.id)}
                              className="h-8 px-3.5 bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs transition-all"
                            >
                              <Play className="w-3.5 h-3.5 fill-current" />
                              继续工时
                            </button>
                            <button
                              onClick={() => handleStop(activeProject.id)}
                              className="h-8 px-3 bg-rose-600 hover:bg-rose-700 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 shadow-xs transition-all"
                            >
                              <Square className="w-3.5 h-3.5 fill-current" />
                              结束工时
                            </button>
                          </div>
                        )}
                      </div>
                    </div>
                  </div>

                  {/* ---------------- 工时控制台与年月筛选栏 ---------------- */}
                  <div className="flex items-center justify-between bg-white border border-slate-200/80 rounded-xl px-4 py-2.5 shadow-sm">
                    {/* 年月控制器 */}
                    <div className="flex items-center gap-2">
                      <div className="flex items-center bg-slate-100 rounded-lg p-0.5 border border-slate-200/60">
                        <button
                          onClick={handlePrevMonth}
                          className="p-1 hover:bg-white rounded-md text-slate-600 transition-colors"
                          title="上个月"
                        >
                          <ChevronLeft className="w-3.5 h-3.5" />
                        </button>
                        <span className="px-2.5 text-xs font-bold text-slate-800 tracking-tight flex items-center gap-1.5">
                          <Calendar className="w-3.5 h-3.5 text-indigo-500" />
                          {selectedMonth}
                        </span>
                        <button
                          onClick={handleNextMonth}
                          className="p-1 hover:bg-white rounded-md text-slate-600 transition-colors"
                          title="下个月"
                        >
                          <ChevronRight className="w-3.5 h-3.5" />
                        </button>
                      </div>

                      <button
                        onClick={() => {
                          const n = new Date();
                          setSelectedMonth(`${n.getFullYear()}-${String(n.getMonth() + 1).padStart(2, "0")}`);
                        }}
                        className="px-2 py-1 text-[11px] font-medium text-slate-500 hover:text-indigo-600 hover:bg-slate-100 rounded-md transition-colors"
                      >
                        本月
                      </button>
                    </div>

                    {/* 视图模式切换与补录动作入口 */}
                    <div className="flex items-center gap-2">
                      {/* 视图切换：流水列表 vs 日历视图 */}
                      <div className="flex items-center bg-slate-100 rounded-lg p-0.5 border border-slate-200/60">
                        <button
                          onClick={() => setViewMode("list")}
                          className={`px-2.5 py-1 rounded-md text-xs font-medium flex items-center gap-1 transition-all ${
                            viewMode === "list"
                              ? "bg-white text-slate-900 shadow-2xs font-semibold"
                              : "text-slate-500 hover:text-slate-900"
                          }`}
                        >
                          <List className="w-3 h-3" />
                          流水
                        </button>
                        <button
                          onClick={() => setViewMode("calendar")}
                          className={`px-2.5 py-1 rounded-md text-xs font-medium flex items-center gap-1 transition-all ${
                            viewMode === "calendar"
                              ? "bg-white text-slate-900 shadow-2xs font-semibold"
                              : "text-slate-500 hover:text-slate-900"
                          }`}
                        >
                          <CalendarDays className="w-3 h-3" />
                          日历
                        </button>
                      </div>

                      <div className="h-4 w-px bg-slate-200 mx-1" />

                      {/* 补录工时入口 */}
                      <button
                        onClick={() => setShowManualEntryModal(true)}
                        className="h-7 px-2.5 rounded-lg border border-slate-200 bg-white hover:bg-slate-50 text-slate-700 text-xs font-medium flex items-center gap-1 shadow-2xs transition-all"
                      >
                        <PlusCircle className="w-3.5 h-3.5 text-indigo-500" />
                        补录工时
                      </button>

                      {/* 补录会议+上传录音入口 */}
                      <button
                        onClick={() => {
                          setManualMeetingPid(activeProject.id);
                          setShowManualMeetingModal(true);
                        }}
                        className="h-7 px-2.5 rounded-lg border border-indigo-200 bg-indigo-50/60 hover:bg-indigo-100 text-indigo-700 text-xs font-medium flex items-center gap-1 shadow-2xs transition-all"
                      >
                        <Upload className="w-3.5 h-3.5 text-indigo-600" />
                        补录会议/录音
                      </button>
                    </div>
                  </div>

                  {/* ---------------- 模式 A：流水列表视图 ---------------- */}
                  {viewMode === "list" && (
                    <div className="space-y-3.5">
                      {dayGroups.length === 0 ? (
                        <div className="bg-white border border-slate-200/90 rounded-xl p-8 text-center space-y-2">
                          <p className="text-xs text-slate-400 font-medium">
                            【{selectedMonth}】该月份暂无工时记录
                          </p>
                          <button
                            onClick={() => setShowManualEntryModal(true)}
                            className="text-xs text-indigo-600 hover:underline font-semibold"
                          >
                            + 点击补录该月工时
                          </button>
                        </div>
                      ) : (
                        dayGroups.map((group) => (
                          <div
                            key={group.date_str}
                            className="bg-white border border-slate-200/80 rounded-xl p-4.5 shadow-sm hover:shadow-md transition-shadow space-y-3"
                          >
                            {/* 每日卡片 Header */}
                            <div className="flex items-center justify-between border-b border-slate-100 pb-2.5">
                              <div className="flex items-center gap-2.5">
                                <span className="text-sm font-bold text-slate-900 flex items-center gap-1.5">
                                  📅 {group.display_date}
                                </span>
                                <span className="px-2 py-0.5 rounded bg-slate-100 text-slate-600 text-[11px] font-medium">
                                  {group.entries_count} 笔工时记录
                                </span>
                              </div>

                              <div className="flex items-center gap-3.5 text-xs">
                                <span className="text-slate-500">投入: <strong className="text-slate-700 font-medium">{group.total_duration}</strong></span>
                                <span className="text-slate-500">折合: <strong className="text-slate-700 font-medium">{group.total_days}</strong></span>
                                {!hideAmount && (
                                  <span className="text-slate-500">价值: <strong className="text-emerald-600 font-semibold">{group.total_amount}</strong></span>
                                )}
                              </div>
                            </div>

                            {/* 当日明细项列表 */}
                            <div className="space-y-2.5">
                              {group.entries.map((entry) => (
                                <div
                                  key={entry.id}
                                  className="p-3.5 rounded-lg bg-slate-50/70 border border-slate-200/60 shadow-2xs space-y-2 hover:bg-slate-50/90 transition-all"
                                >
                                  <div className="flex items-center justify-between text-xs">
                                    <div className="flex items-center gap-3 text-slate-600">
                                      <span className="font-semibold text-slate-900 flex items-center gap-1">
                                        <Clock className="w-3.5 h-3.5 text-slate-400" />
                                        {entry.time_range}
                                      </span>
                                      <span>投入: {entry.duration_text}</span>
                                      <span>折算: {entry.days_text}</span>
                                      {!hideAmount && (
                                        <span className="text-emerald-600 font-semibold">{entry.amount_text}</span>
                                      )}
                                    </div>

                                    {/* 结算状态标签 (可点击直接切换) */}
                                    <button
                                      onClick={() => handleToggleEntryInvoiced(entry.id, entry.is_invoiced)}
                                      title={entry.is_invoiced ? "点击重置为待结算" : "点击标记为已结算对账"}
                                      className={`px-2 py-0.5 rounded-full text-[10px] font-bold flex items-center gap-1 border transition-all active:scale-95 ${
                                        entry.is_invoiced
                                          ? "bg-emerald-50 text-emerald-700 border-emerald-300 hover:bg-emerald-100"
                                          : "bg-amber-50 text-amber-700 border-amber-300 hover:bg-amber-100"
                                      }`}
                                    >
                                      {entry.is_invoiced ? (
                                        <>
                                          <CheckCheck className="w-3 h-3 text-emerald-600" />
                                          已结算
                                        </>
                                      ) : (
                                        <>
                                          <span className="w-1.5 h-1.5 rounded-full bg-amber-500 animate-pulse" />
                                          待结算
                                        </>
                                      )}
                                    </button>
                                  </div>

                                  <div
                                    className="markdown-body text-xs text-slate-700 leading-relaxed font-normal"
                                    dangerouslySetInnerHTML={{ __html: renderMarkdown(entry.summary_text) }}
                                  />

                                  {/* 操作按钮区 */}
                                  <div className="flex items-center justify-between pt-1">
                                    {entry.has_audio ? (
                                      <div className="flex items-center gap-2">
                                        <button
                                          onClick={() => invoke("play_audio", { audioPath: entry.audio_path })}
                                          className="h-6.5 px-2.5 rounded bg-slate-200/80 hover:bg-slate-300 text-slate-700 text-[11px] font-medium flex items-center gap-1 transition-all"
                                        >
                                          <Volume2 className="w-3 h-3 text-slate-600" />
                                          播放录音
                                        </button>
                                        <button
                                          onClick={() => handleTranscribe(entry)}
                                          className="h-6.5 px-2.5 rounded bg-indigo-50 border border-indigo-200 text-indigo-700 hover:bg-indigo-100 text-[11px] font-medium flex items-center gap-1 transition-all"
                                        >
                                          <Sparkles className="w-3 h-3 text-indigo-600" />
                                          AI转写纪要
                                        </button>
                                      </div>
                                    ) : (
                                      <div />
                                    )}

                                    <button
                                      onClick={() => {
                                        setSelectedEntry(entry);
                                        setShowDetailModal(true);
                                      }}
                                      className="h-6.5 px-2.5 rounded bg-white border border-slate-200 text-slate-600 hover:bg-slate-50 text-[11px] font-medium flex items-center gap-1 shadow-2xs"
                                    >
                                      <FileText className="w-3 h-3 text-slate-400" />
                                      查看文稿详情
                                    </button>
                                  </div>
                                </div>
                              ))}
                            </div>
                          </div>
                        ))
                      )}
                    </div>
                  )}

                  {/* ---------------- 模式 B：月历视图 (Calendar View) ---------------- */}
                  {viewMode === "calendar" && (() => {
                    const [yearNum, monthNum] = selectedMonth.split("-").map(Number);
                    const firstDayOfWeek = new Date(yearNum, monthNum - 1, 1).getDay(); // 0 is Sunday
                    const totalDaysInMonth = new Date(yearNum, monthNum, 0).getDate();
                    // 周一到周日偏移 (0->6, 1->0, 2->1 ...)
                    const offset = (firstDayOfWeek + 6) % 7;
                    const weekHeaders = ["周一", "周二", "周三", "周四", "周五", "周六", "周日"];
                    
                    const groupDateMap = new Map(dayGroups.map((g) => [g.date_str, g]));
                    const selectedDayGroup = calendarSelectedDate ? groupDateMap.get(calendarSelectedDate) : null;

                    return (
                      <div className="space-y-4">
                        <div className="bg-white border border-slate-200/90 rounded-xl p-4 shadow-xs">
                          {/* 星期表头 */}
                          <div className="grid grid-cols-7 gap-2 mb-2 text-center text-xs font-bold text-slate-500 pb-2 border-b border-slate-100">
                            {weekHeaders.map((w, idx) => (
                              <div key={w} className={idx >= 5 ? "text-amber-600" : ""}>{w}</div>
                            ))}
                          </div>

                          {/* 月历日期网格 */}
                          <div className="grid grid-cols-7 gap-2">
                            {/* 前置留白 */}
                            {Array.from({ length: offset }).map((_, i) => (
                              <div key={`empty-${i}`} className="h-24 rounded-lg bg-slate-50/40 border border-transparent" />
                            ))}

                            {/* 本月各天 */}
                            {Array.from({ length: totalDaysInMonth }).map((_, i) => {
                              const dayNum = i + 1;
                              const dStr = `${yearNum}-${String(monthNum).padStart(2, "0")}-${String(dayNum).padStart(2, "0")}`;
                              const group = groupDateMap.get(dStr);
                              const isToday = dStr === new Date().toISOString().split("T")[0];
                              const isSelected = calendarSelectedDate === dStr;
                              const hasAudio = group?.entries.some((e) => e.has_audio);

                              return (
                                <div
                                  key={dStr}
                                  onClick={() => setCalendarSelectedDate(dStr === calendarSelectedDate ? null : dStr)}
                                  className={`h-24 p-2 rounded-lg border transition-all cursor-pointer flex flex-col justify-between ${
                                    isSelected
                                      ? "ring-2 ring-indigo-500 border-indigo-500 bg-indigo-50/40"
                                      : group
                                      ? "bg-white hover:bg-slate-50 border-slate-200/90 shadow-2xs"
                                      : "bg-slate-50/60 hover:bg-slate-100/60 border-slate-100 text-slate-400"
                                  }`}
                                >
                                  <div className="flex items-center justify-between">
                                    <span
                                      className={`text-xs font-bold w-5 h-5 flex items-center justify-center rounded-full ${
                                        isToday
                                          ? "bg-indigo-600 text-white"
                                          : group
                                          ? "text-slate-800"
                                          : "text-slate-400"
                                      }`}
                                    >
                                      {dayNum}
                                    </span>
                                    {hasAudio && (
                                      <span title="包含会议/录音凭证">
                                        <Volume2 className="w-3 h-3 text-indigo-500" />
                                      </span>
                                    )}
                                  </div>

                                  {group ? (
                                    <div className="space-y-0.5 text-left">
                                      <div className="text-[11px] font-bold text-slate-900 leading-tight">
                                        {group.total_duration}
                                      </div>
                                      <div className="text-[10px] text-slate-500">
                                        {group.total_days}
                                      </div>
                                      {!hideAmount && (
                                        <div className="text-[10px] font-semibold text-emerald-600 truncate">
                                          {group.total_amount}
                                        </div>
                                      )}
                                    </div>
                                  ) : (
                                    <div className="text-[10px] text-slate-300 text-center py-2">
                                      无工时
                                    </div>
                                  )}
                                </div>
                              );
                            })}
                          </div>
                        </div>

                        {/* 点击日历某一天时展开的流水明细抽屉卡片 */}
                        {calendarSelectedDate && (
                          <div className="bg-white border border-slate-200/90 rounded-xl p-4 shadow-xs space-y-3 animate-in fade-in duration-150">
                            <div className="flex items-center justify-between border-b border-slate-100 pb-2.5">
                              <div className="flex items-center gap-2">
                                <span className="text-sm font-bold text-slate-900">
                                  📅 {calendarSelectedDate} 工时明细
                                </span>
                                {selectedDayGroup && (
                                  <span className="text-xs text-slate-500">
                                    （共 {selectedDayGroup.entries_count} 条 · {selectedDayGroup.total_duration}{!hideAmount ? ` · ${selectedDayGroup.total_amount}` : ""}）
                                  </span>
                                )}
                              </div>
                              <button
                                onClick={() => setCalendarSelectedDate(null)}
                                className="text-xs text-slate-400 hover:text-slate-700"
                              >
                                ✕ 收起
                              </button>
                            </div>

                            {!selectedDayGroup || selectedDayGroup.entries.length === 0 ? (
                              <div className="p-4 text-center text-xs text-slate-400">
                                当日暂无工时流水，
                                <button
                                  onClick={() => {
                                    setManualEntryDate(calendarSelectedDate);
                                    setShowManualEntryModal(true);
                                  }}
                                  className="text-indigo-600 font-semibold hover:underline ml-1"
                                >
                                  + 立即补录
                                </button>
                              </div>
                            ) : (
                              <div className="space-y-2">
                                {selectedDayGroup.entries.map((entry) => (
                                  <div
                                    key={entry.id}
                                    className="p-3 rounded-lg bg-slate-50/60 border border-slate-100 space-y-1.5"
                                  >
                                    <div className="flex items-center justify-between text-xs">
                                      <div className="flex items-center gap-3 text-slate-600">
                                        <span className="font-semibold text-slate-900 flex items-center gap-1">
                                          <Clock className="w-3.5 h-3.5 text-slate-400" />
                                          {entry.time_range}
                                        </span>
                                        <span>投入: {entry.duration_text}</span>
                                        <span>折算: {entry.days_text}</span>
                                        {!hideAmount && (
                                          <span className="text-emerald-600 font-semibold">{entry.amount_text}</span>
                                        )}
                                      </div>

                                      <button
                                        onClick={() => handleToggleEntryInvoiced(entry.id, entry.is_invoiced)}
                                        title={entry.is_invoiced ? "点击重置为待结算" : "点击标记为已结算对账"}
                                        className={`px-2 py-0.5 rounded-full text-[10px] font-bold flex items-center gap-1 border transition-all active:scale-95 ${
                                          entry.is_invoiced
                                            ? "bg-emerald-50 text-emerald-700 border-emerald-300 hover:bg-emerald-100"
                                            : "bg-amber-50 text-amber-700 border-amber-300 hover:bg-amber-100"
                                        }`}
                                      >
                                        {entry.is_invoiced ? "已结算" : "待结算"}
                                      </button>
                                    </div>
                                    <div
                                      className="markdown-body text-xs text-slate-700 leading-relaxed font-normal"
                                      dangerouslySetInnerHTML={{ __html: renderMarkdown(entry.summary_text) }}
                                    />
                                    {entry.has_audio && (
                                      <div className="pt-1">
                                        <button
                                          onClick={() => invoke("play_audio", { audioPath: entry.audio_path })}
                                          className="h-6 px-2 rounded bg-slate-200/80 hover:bg-slate-300 text-slate-700 text-[11px] font-medium flex items-center gap-1"
                                        >
                                          <Volume2 className="w-3 h-3 text-slate-600" />
                                          播放录音
                                        </button>
                                      </div>
                                    )}
                                  </div>
                                ))}
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    );
                  })()}
                </>
              )}
            </div>
          </div>
        )}

        {/* 选项卡视图：统计报表与多维对账结算导出 (重构：清爽列表 + 紧凑条件栏 + 弹窗配置) */}
        {activeTab === "stats" && (
          <div className="flex-1 p-6 overflow-y-auto space-y-4">
            {/* 1. 顶部指标概览栏 */}
            <div className={`grid gap-4 ${hideAmount ? "grid-cols-2" : "grid-cols-4"}`}>
              <div className="bg-white border border-slate-200/80 rounded-xl p-4 shadow-2xs">
                <div className="text-[11px] font-medium text-slate-400">核算总投入工时</div>
                <div className="text-xl font-extrabold text-slate-900 mt-1">
                  {settlementReport ? settlementReport.total_days.toFixed(2) : "0.00"}{" "}
                  <span className="text-xs font-normal text-slate-500">人天</span>
                </div>
              </div>
              <div className="bg-white border border-slate-200/80 rounded-xl p-4 shadow-2xs">
                <div className="text-[11px] font-medium text-slate-400">核算对账记录条数</div>
                <div className="text-xl font-extrabold text-indigo-600 mt-1">
                  {settlementReport ? settlementReport.entries.length : 0}{" "}
                  <span className="text-xs font-normal text-slate-500">笔工时</span>
                </div>
              </div>
              {!hideAmount && (
                <>
                  <div className="bg-white border border-slate-200/80 rounded-xl p-4 shadow-2xs">
                    <div className="text-[11px] font-medium text-slate-400">总核算金额</div>
                    <div className="text-xl font-extrabold text-indigo-600 mt-1">
                      ¥ {settlementReport ? settlementReport.total_amount.toFixed(2) : "0.00"}
                    </div>
                  </div>
                  <div className="bg-white border border-emerald-200/80 bg-emerald-50/15 rounded-xl p-4 shadow-2xs">
                    <div className="text-[11px] font-medium text-emerald-600">已结算 / 已回款</div>
                    <div className="text-xl font-extrabold text-emerald-600 mt-1">
                      ¥ {settlementReport ? settlementReport.settled_amount.toFixed(2) : "0.00"}
                    </div>
                  </div>
                </>
              )}
            </div>

            {/* 2. 紧凑单行查询条件与操作栏 */}
            <div className="bg-white border border-slate-200/80 rounded-xl px-4 py-3 shadow-2xs flex items-center justify-between gap-4">
              <div className="flex items-center gap-3">
                {/* 状态快捷切换 */}
                <div className="flex items-center bg-slate-100 rounded-lg p-0.5 border border-slate-200/60 text-xs font-medium">
                  <button
                    onClick={() => {
                      setReportStatusFilter("all");
                      setTimeout(fetchSettlementReport, 50);
                    }}
                    className={`px-3 py-1 rounded-md transition-all ${
                      reportStatusFilter === "all" ? "bg-white text-slate-900 font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    全部状态
                  </button>
                  <button
                    onClick={() => {
                      setReportStatusFilter("pending");
                      setTimeout(fetchSettlementReport, 50);
                    }}
                    className={`px-3 py-1 rounded-md transition-all ${
                      reportStatusFilter === "pending" ? "bg-amber-500 text-white font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    待结算
                  </button>
                  <button
                    onClick={() => {
                      setReportStatusFilter("settled");
                      setTimeout(fetchSettlementReport, 50);
                    }}
                    className={`px-3 py-1 rounded-md transition-all ${
                      reportStatusFilter === "settled" ? "bg-emerald-600 text-white font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    已结算
                  </button>
                </div>

                <div className="h-4 w-px bg-slate-200" />

                {/* 筛选摘要标签与弹窗触发器 */}
                <button
                  onClick={() => setShowFilterModal(true)}
                  className="h-8 px-3 rounded-lg border border-slate-200 hover:bg-slate-50 text-xs font-medium text-slate-700 flex items-center gap-1.5 transition-all"
                >
                  <Filter className="w-3.5 h-3.5 text-indigo-600" />
                  <span>筛选条件：</span>
                  <span className="font-bold text-slate-900">
                    {reportSelectedMonths.length > 0 ? `${reportSelectedMonths.length} 个月份` : "全部月份"}
                    {" · "}
                    {reportSelectedProjects.length > 0 ? `${reportSelectedProjects.length} 个项目` : "全部项目"}
                  </span>
                </button>

                <div className="h-4 w-px bg-slate-200" />

                {/* 人天结算策略选择器 (4大商业核算策略) */}
                <div className="flex items-center gap-1.5 bg-slate-50 border border-slate-200/90 rounded-lg p-0.5">
                  <span className="text-[11px] font-bold text-slate-500 pl-2 pr-1 flex items-center gap-1">
                    <span>⚖️</span> 核算策略:
                  </span>
                  <select
                    value={currentStrategy}
                    onChange={(e) => {
                      const newStrat = e.target.value as "rebalance" | "min8_carry" | "cap8_carry" | "exact";
                      setCurrentStrategy(newStrat);
                      fetchSettlementReport(newStrat);
                      const stratLabels: Record<string, string> = {
                        rebalance: "🛡️ 已切换为：合规削峰填谷 (8h封顶·工作日平摊)",
                        min8_carry: "💎 已切换为：保底8小时 (不足8h按8h·超额顺延新天)",
                        cap8_carry: "⏩ 已切换为：封顶顺延 (单日封顶8h·超额顺延新天)",
                        exact: "⏱️ 已切换为：实际物理工时 (实报实销·同日合并)",
                      };
                      showToast(stratLabels[newStrat]);
                    }}
                    className="h-7 text-xs font-bold text-indigo-700 bg-white border border-indigo-200/70 rounded-md px-2 focus:outline-none focus:ring-1 focus:ring-indigo-500 cursor-pointer shadow-2xs"
                  >
                    <option value="rebalance">🛡️ 合规削峰填谷 (8h/天上限·平摊)</option>
                    <option value="min8_carry">💎 保底8小时模式 (不足8h按8h·超额顺延)</option>
                    <option value="cap8_carry">⏩ 封顶顺延模式 (单日8h封顶·顺延新天)</option>
                    <option value="exact">⏱️ 实际物理工时 (同日合并·实报实销)</option>
                  </select>
                </div>
              </div>

              {/* 右侧操作按钮 */}
              <div className="flex items-center gap-2.5">
                <button
                  onClick={() => handleBatchToggleInvoiced(true)}
                  className="h-8 px-3 rounded-lg bg-emerald-50 hover:bg-emerald-100 border border-emerald-200 text-emerald-700 text-xs font-bold flex items-center gap-1 transition-all active:scale-95"
                  title="将当前列表匹配的全部工时标记为已结算"
                >
                  <CheckCheck className="w-3.5 h-3.5 text-emerald-600" />
                  本期全标已结
                </button>
                <button
                  onClick={() => handleBatchToggleInvoiced(false)}
                  className="h-8 px-2.5 rounded-lg border border-slate-200 hover:bg-slate-50 text-slate-600 text-xs font-medium transition-all"
                  title="重置为待结算"
                >
                  重置待结
                </button>

                <div className="h-4 w-px bg-slate-200" />

                <button
                  onClick={() => {
                    setExportSelectedProjects(reportSelectedProjects.length > 0 ? reportSelectedProjects : projects.map(p => p.id));
                    setExportSelectedMonths(reportSelectedMonths);
                    setShowExportSettingsModal(true);
                  }}
                  className="h-8 px-3 rounded-lg bg-emerald-600 hover:bg-emerald-700 text-white text-xs font-bold flex items-center gap-1.5 shadow-xs transition-all active:scale-95 cursor-pointer"
                  title="多选项目、多月份、独立文件与按月Sheet专业导出"
                >
                  <Download className="w-3.5 h-3.5" />
                  导出 Excel
                </button>

                <button
                  onClick={() => setShowReportExportModal(true)}
                  className="h-8 px-3 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-bold flex items-center gap-1.5 shadow-xs transition-all active:scale-95"
                >
                  <FileSpreadsheet className="w-3.5 h-3.5" />
                  对账单预览
                </button>
              </div>
            </div>

            {/* 3. 标准数据表格/工时流水列表 */}
            <div className="bg-white border border-slate-200/80 rounded-xl shadow-2xs overflow-hidden">
              <div className="px-4 py-3 border-b border-slate-100 flex items-center justify-between bg-slate-50/50">
                <div className="text-xs font-bold text-slate-700 flex items-center gap-2">
                  <span>对账明细流水</span>
                  <span className="text-[11px] font-normal text-slate-400">
                    （共 {settlementReport?.entries ? settlementReport.entries.length : 0} 笔记录）
                  </span>
                </div>
                <div className="text-[11px] text-slate-400">
                  提示：点击每行的「结算状态」按钮可快速切换状态
                </div>
              </div>

              {!settlementReport || settlementReport.entries.length === 0 ? (
                <div className="p-12 text-center space-y-2">
                  <div className="text-slate-300 text-3xl">📭</div>
                  <div className="text-xs text-slate-400">当前筛选条件下无工时明细流水</div>
                </div>
              ) : (
                <div className="overflow-x-auto max-h-[calc(100vh-290px)] overflow-y-auto">
                  <table className="w-full text-left border-collapse text-xs">
                    <thead className="bg-slate-50 sticky top-0 z-10 text-[11px] font-bold text-slate-500 uppercase tracking-wider border-b border-slate-200/80">
                      <tr>
                        <th className="py-2.5 px-4">日期与时间</th>
                        <th className="py-2.5 px-4">归属项目</th>
                        <th className="py-2.5 px-4">投入工时 (h)</th>
                        <th className="py-2.5 px-4">折算人天 (天)</th>
                        {!hideAmount && <th className="py-2.5 px-4">核算金额 (元)</th>}
                        <th className="py-2.5 px-4">工作内容 / 交付纪要</th>
                        <th className="py-2.5 px-4 text-center">结算状态</th>
                        <th className="py-2.5 px-4 text-right">操作</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-slate-100 text-slate-700">
                      {settlementReport.entries.map((entry) => (
                        <tr key={entry.id} className="hover:bg-slate-50/70 transition-colors">
                          <td className="py-2.5 px-4 font-mono text-[11px] whitespace-nowrap text-slate-600">
                            <span className="font-bold text-slate-800">{entry.date_str}</span>
                            <span className="ml-1.5 text-slate-400">{entry.time_range}</span>
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap font-medium text-slate-900">
                            {entry.project_name}
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap text-slate-700 font-bold font-mono">
                            {formatToHalfHours(entry.duration_text)}
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap font-semibold text-slate-800 font-mono">
                            {formatPureNumber(entry.days_text)}
                          </td>
                          {!hideAmount && (
                            <td className="py-2.5 px-4 whitespace-nowrap font-bold text-emerald-600 font-mono">
                              {formatPureNumber(entry.amount_text)}
                            </td>
                          )}
                          <td className="py-2.5 px-4 max-w-xs truncate text-slate-600" title={entry.summary_text}>
                            {entry.summary_text}
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap text-center">
                            <button
                              onClick={() => handleToggleEntryInvoiced(entry.id, entry.is_invoiced)}
                              className={`px-2 py-0.5 rounded-full text-[10px] font-bold inline-flex items-center gap-1 border transition-all active:scale-95 ${
                                entry.is_invoiced
                                  ? "bg-emerald-50 text-emerald-700 border-emerald-300 hover:bg-emerald-100"
                                  : "bg-amber-50 text-amber-700 border-amber-300 hover:bg-amber-100"
                              }`}
                              title={entry.is_invoiced ? "点击重置为待结算" : "点击标记为已结算"}
                            >
                              {entry.is_invoiced ? (
                                <>
                                  <CheckCheck className="w-3 h-3 text-emerald-600" />
                                  已结算
                                </>
                              ) : (
                                <>
                                  <span className="w-1.5 h-1.5 rounded-full bg-amber-500 animate-pulse" />
                                  待结算
                                </>
                              )}
                            </button>
                          </td>
                          <td className="py-2.5 px-4 whitespace-nowrap text-right">
                            <button
                              onClick={() => {
                                setSelectedEntry(entry);
                                setShowDetailModal(true);
                              }}
                              className="text-xs text-indigo-600 hover:underline font-medium"
                            >
                              查看详情
                            </button>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}
            </div>
          </div>
        )}

        {/* 选项卡视图：系统与AI引擎配置 */}
        {activeTab === "settings" && (
          <div className="flex-1 p-6 overflow-y-auto">
            <div className="max-w-xl bg-white border border-slate-200/80 rounded-2xl p-6 shadow-sm space-y-5">
              <h3 className="text-lg font-extrabold text-slate-900">⚙️ AI 引擎与工时规则配置</h3>
              <p className="text-xs text-slate-500">
                支持开箱即用免 Key 的本地 agy / codex CLI，以及兼容 OpenAI / DeepSeek / Ollama 的自定义 API
              </p>
              <div className="p-4 rounded-xl bg-emerald-50 border border-emerald-200 text-emerald-800 text-xs font-medium leading-relaxed">
                ✨ 当前默认开启 <strong>本地已登录的 agy CLI 模式</strong>。无需填写 API Key，MythManDay 自动完成语音转写与交付纪要提炼！
              </div>
            </div>
          </div>
        )}
      </main>

      {/* ---------------- 模态框 1: 发起会议沟通 ---------------- */}
      {showMeetingModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-200">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900 flex items-center gap-2">
                <Mic className="w-4 h-4 text-blue-600" />
                发起即时会议与讨论
              </h3>
              <button
                onClick={() => setShowMeetingModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">会议议题 / 讨论内容</label>
                <input
                  type="text"
                  value={meetingTopic}
                  onChange={(e) => setMeetingTopic(e.target.value)}
                  placeholder="例如：客户方案需求评审、架构重构对齐会"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-2 focus:ring-blue-500/20 focus:border-blue-500"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">关联归属项目</label>
                <select
                  value={meetingSelectedPid}
                  onChange={(e) => setMeetingSelectedPid(e.target.value)}
                  className="w-full h-9 px-2 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                >
                  <option value="">⚡ 独立临时会议 (单独立项 · 不计入既有客户)</option>
                  {projects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} ({p.client})
                    </option>
                  ))}
                </select>
              </div>

              {/* 费率自动继承提示 */}
              <div className="p-2.5 rounded-lg bg-slate-50 border border-slate-200/80 text-[11px] text-slate-600 flex items-center justify-between">
                <span>💰 计费费率：</span>
                <span className="font-semibold text-slate-800">
                  {meetingSelectedPid
                    ? `${projects.find((p) => p.id === meetingSelectedPid)?.rate_info || "继承项目既定标准"}`
                    : "独立临时咨询 (标准: 350 CNY/小时)"}
                </span>
              </div>

              <div className="flex items-center gap-2 pt-2">
                <input
                  type="checkbox"
                  id="recordAudio"
                  checked={meetingRecordAudio}
                  onChange={(e) => setMeetingRecordAudio(e.target.checked)}
                  className="w-4 h-4 rounded text-blue-600"
                />
                <label htmlFor="recordAudio" className="text-xs font-medium text-slate-700">
                  🎙️ 同步开启麦克风全程录音（直存本地高保真 WAV）
                </label>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowMeetingModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleStartMeeting}
                className="px-4 py-2 text-xs font-bold bg-blue-600 hover:bg-blue-700 text-white rounded-lg shadow-sm transition-all"
              >
                开始会议计时
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 2: 新建项目 ---------------- */}
      {showNewProjModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900">➕ 新建研发 / 计费项目</h3>
              <button
                onClick={() => setShowNewProjModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">项目名称 (必填)</label>
                <input
                  type="text"
                  value={newProjName}
                  onChange={(e) => setNewProjName(e.target.value)}
                  placeholder="例如：客户A 跨境商城二次重构"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">客户 / 委托方</label>
                <input
                  type="text"
                  value={newProjClient}
                  onChange={(e) => setNewProjClient(e.target.value)}
                  placeholder="例如：某某科技有限公司 / 张总"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                />
              </div>

              {/* 商业计费规则配置卡片 */}
              <div className="p-3 bg-slate-50 border border-slate-200/80 rounded-xl space-y-3">
                <div className="flex items-center justify-between pb-1 border-b border-slate-200/60">
                  <span className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                    <span>💳</span> 商业计费规则
                  </span>
                  <span className="text-[10px] text-indigo-600 font-medium">支持灵活结算策略</span>
                </div>

                {/* 1. 计费方式切换 */}
                <div>
                  <label className="text-[11px] font-bold text-slate-600 block mb-1">计费方式</label>
                  <div className="grid grid-cols-2 gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        setNewProjBillingType("daily");
                        if (newProjRate === "400") setNewProjRate("3000");
                      }}
                      className={`h-8 px-2.5 rounded-lg border text-xs font-bold flex items-center justify-center gap-1.5 transition-all ${
                        newProjBillingType === "daily"
                          ? "bg-indigo-600 border-indigo-600 text-white shadow-xs"
                          : "bg-white border-slate-200 text-slate-700 hover:bg-slate-100"
                      }`}
                    >
                      <span>📅</span> 按天 (默认最小0.5天)
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setNewProjBillingType("hourly");
                        if (newProjRate === "3000") setNewProjRate("400");
                      }}
                      className={`h-8 px-2.5 rounded-lg border text-xs font-bold flex items-center justify-center gap-1.5 transition-all ${
                        newProjBillingType === "hourly"
                          ? "bg-indigo-600 border-indigo-600 text-white shadow-xs"
                          : "bg-white border-slate-200 text-slate-700 hover:bg-slate-100"
                      }`}
                    >
                      <span>⏱️</span> 按小时
                    </button>
                  </div>
                </div>

                {/* 2. 价格与币种 */}
                <div className="grid grid-cols-2 gap-2.5">
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">
                      {newProjBillingType === "daily" ? "人天单价 (价格/天)" : "时薪单价 (价格/小时)"}
                    </label>
                    <input
                      type="number"
                      value={newProjRate}
                      onChange={(e) => setNewProjRate(e.target.value)}
                      placeholder={newProjBillingType === "daily" ? "3000" : "400"}
                      className="w-full h-8 px-3 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-mono"
                    />
                  </div>
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">结算币种</label>
                    <select
                      value={newProjCurrency}
                      onChange={(e) => setNewProjCurrency(e.target.value)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-medium"
                    >
                      <option value="CNY">CNY (人民币 ¥)</option>
                      <option value="USD">USD (美元 $)</option>
                      <option value="EUR">EUR (欧元 €)</option>
                      <option value="HKD">HKD (港币 HK$)</option>
                      <option value="JPY">JPY (日元 ¥)</option>
                      <option value="GBP">GBP (英镑 £)</option>
                    </select>
                  </div>
                </div>

                {/* 3. 舍入方式与最小刻度 */}
                <div className="grid grid-cols-2 gap-2.5">
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">舍入方式</label>
                    <select
                      value={newProjRoundingMode}
                      onChange={(e) => setNewProjRoundingMode(e.target.value as any)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                    >
                      <option value="ceil">进位 (不足进1/向上取整)</option>
                      <option value="half_up">四舍五入 (按刻度标准舍入)</option>
                      <option value="floor">舍去 (向下截断)</option>
                    </select>
                  </div>
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">
                      最小刻度 ({newProjBillingType === "daily" ? "天" : "小时"})
                    </label>
                    <select
                      value={newProjMinGranularity}
                      onChange={(e) => setNewProjMinGranularity(e.target.value)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-mono"
                    >
                      <option value="0.5">0.5 {newProjBillingType === "daily" ? "天 (半天制)" : "小时 (半小时)"}</option>
                      <option value="1.0">1.0 {newProjBillingType === "daily" ? "天 (整天制)" : "小时 (整小时)"}</option>
                      <option value="0.25">0.25 {newProjBillingType === "daily" ? "天 (2小时)" : "小时 (15分钟)"}</option>
                      <option value="0.1">0.1 {newProjBillingType === "daily" ? "天 (精确制)" : "小时 (6分钟)"}</option>
                    </select>
                  </div>
                </div>
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">本地 Git 仓库路径 (可选，智能嗅探代码)</label>
                <input
                  type="text"
                  value={newProjGit}
                  onChange={(e) => setNewProjGit(e.target.value)}
                  placeholder="/home/beatle/dev/my-project"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowNewProjModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors cursor-pointer"
              >
                取消
              </button>
              <button
                onClick={handleCreateProject}
                className="px-4 py-2 text-xs font-bold bg-blue-600 hover:bg-blue-700 text-white rounded-lg shadow-sm transition-all cursor-pointer"
              >
                保存并创建项目
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 2.5: 配置与修改老项目商业计费规则 ---------------- */}
      {showEditProjModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-100">
          <div className="w-full max-w-lg bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-3 border-b border-slate-100">
              <div className="flex items-center gap-2.5">
                <div className="w-9 h-9 rounded-xl bg-indigo-50 border border-indigo-100 flex items-center justify-center text-indigo-600 text-lg">
                  ⚙️
                </div>
                <div>
                  <h3 className="text-base font-extrabold text-slate-900 tracking-tight">配置项目商业计费规则</h3>
                  <p className="text-xs text-slate-500 font-medium">调整计费模式、费率、币种、舍入方式与阶梯刻度</p>
                </div>
              </div>
              <button
                onClick={() => setShowEditProjModal(false)}
                className="w-7 h-7 flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-100 rounded-lg text-sm font-bold transition-colors cursor-pointer"
              >
                ✕
              </button>
            </div>

            <div className="space-y-4">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">项目名称</label>
                <input
                  type="text"
                  value={editProjName}
                  onChange={(e) => setEditProjName(e.target.value)}
                  placeholder="例如：智能客服大模型中台架构研发"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-medium"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">客户 / 委托方名称</label>
                <input
                  type="text"
                  value={editProjClient}
                  onChange={(e) => setEditProjClient(e.target.value)}
                  placeholder="例如：北京某智能科技集团 / 某出海团队"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-medium"
                />
              </div>

              {/* 商业计费与结算规则 */}
              <div className="p-3.5 bg-slate-50/80 rounded-xl border border-slate-200/80 space-y-3">
                <div className="flex items-center justify-between border-b border-slate-200/60 pb-2">
                  <span className="text-xs font-extrabold text-slate-800 flex items-center gap-1.5">
                    <span>⚖️</span> 计费规则与精细阶梯控制
                  </span>
                  <span className="text-[10px] text-indigo-600 bg-indigo-50 px-1.5 py-0.5 rounded font-mono font-bold">
                    实时生效
                  </span>
                </div>

                {/* 1. 计费方式切换 */}
                <div>
                  <label className="text-[11px] font-bold text-slate-600 block mb-1">计费方式</label>
                  <div className="grid grid-cols-2 gap-2">
                    <button
                      type="button"
                      onClick={() => {
                        setEditProjBillingType("daily");
                        if (editProjRate === "400") setEditProjRate("3000");
                      }}
                      className={`h-8 px-2.5 rounded-lg border text-xs font-bold flex items-center justify-center gap-1.5 transition-all cursor-pointer ${
                        editProjBillingType === "daily"
                          ? "bg-indigo-600 border-indigo-600 text-white shadow-xs"
                          : "bg-white border-slate-200 text-slate-700 hover:bg-slate-100"
                      }`}
                    >
                      <span>📅</span> 按天 (默认最小0.5天)
                    </button>
                    <button
                      type="button"
                      onClick={() => {
                        setEditProjBillingType("hourly");
                        if (editProjRate === "3000") setEditProjRate("400");
                      }}
                      className={`h-8 px-2.5 rounded-lg border text-xs font-bold flex items-center justify-center gap-1.5 transition-all cursor-pointer ${
                        editProjBillingType === "hourly"
                          ? "bg-indigo-600 border-indigo-600 text-white shadow-xs"
                          : "bg-white border-slate-200 text-slate-700 hover:bg-slate-100"
                      }`}
                    >
                      <span>⏱️</span> 按小时
                    </button>
                  </div>
                </div>

                {/* 2. 价格与币种 */}
                <div className="grid grid-cols-2 gap-2.5">
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">
                      {editProjBillingType === "daily" ? "人天单价 (价格/天)" : "时薪单价 (价格/小时)"}
                    </label>
                    <input
                      type="number"
                      value={editProjRate}
                      onChange={(e) => setEditProjRate(e.target.value)}
                      placeholder={editProjBillingType === "daily" ? "3000" : "400"}
                      className="w-full h-8 px-3 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-mono"
                    />
                  </div>
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">结算币种</label>
                    <select
                      value={editProjCurrency}
                      onChange={(e) => setEditProjCurrency(e.target.value)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-medium"
                    >
                      <option value="CNY">CNY (人民币 ¥)</option>
                      <option value="USD">USD (美元 $)</option>
                      <option value="EUR">EUR (欧元 €)</option>
                      <option value="HKD">HKD (港币 HK$)</option>
                      <option value="JPY">JPY (日元 ¥)</option>
                      <option value="GBP">GBP (英镑 £)</option>
                    </select>
                  </div>
                </div>

                {/* 3. 舍入方式与最小刻度 */}
                <div className="grid grid-cols-2 gap-2.5">
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">舍入方式</label>
                    <select
                      value={editProjRoundingMode}
                      onChange={(e) => setEditProjRoundingMode(e.target.value as any)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                    >
                      <option value="ceil">进位 (不足进1/向上取整)</option>
                      <option value="half_up">四舍五入 (按刻度标准舍入)</option>
                      <option value="floor">舍去 (向下截断)</option>
                    </select>
                  </div>
                  <div>
                    <label className="text-[11px] font-bold text-slate-600 block mb-1">
                      最小刻度 ({editProjBillingType === "daily" ? "天" : "小时"})
                    </label>
                    <select
                      value={editProjMinGranularity}
                      onChange={(e) => setEditProjMinGranularity(e.target.value)}
                      className="w-full h-8 px-2 rounded-lg border border-slate-200 bg-white text-xs text-slate-800 focus:outline-none focus:border-indigo-500 font-mono"
                    >
                      <option value="0.5">0.5 {editProjBillingType === "daily" ? "天 (半天制)" : "小时 (半小时)"}</option>
                      <option value="1.0">1.0 {editProjBillingType === "daily" ? "天 (整天制)" : "小时 (整小时)"}</option>
                      <option value="0.25">0.25 {editProjBillingType === "daily" ? "天 (2小时)" : "小时 (15分钟)"}</option>
                      <option value="0.1">0.1 {editProjBillingType === "daily" ? "天 (精确制)" : "小时 (6分钟)"}</option>
                    </select>
                  </div>
                </div>
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">本地 Git 仓库路径 (可选，智能嗅探代码)</label>
                <input
                  type="text"
                  value={editProjGit}
                  onChange={(e) => setEditProjGit(e.target.value)}
                  placeholder="/home/beatle/dev/my-project"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:border-indigo-500"
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowEditProjModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors cursor-pointer"
              >
                取消
              </button>
              <button
                onClick={handleUpdateProject}
                className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm transition-all cursor-pointer"
              >
                保存规则配置
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 3: 结构化单日工时与工作交付详情 (选项 A) ---------------- */}
      {showDetailModal && selectedEntry && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-xl bg-white rounded-2xl border border-slate-200/90 shadow-2xl p-6 space-y-5 animate-in fade-in zoom-in duration-150">
            {/* 顶部标题栏与关闭 */}
            <div className="flex items-center justify-between pb-3 border-b border-slate-100">
              <div className="flex items-center gap-2.5">
                <div className="w-9 h-9 rounded-xl bg-indigo-50 border border-indigo-100 flex items-center justify-center text-indigo-600 text-lg">
                  📅
                </div>
                <div>
                  <h3 className="text-base font-extrabold text-slate-900 tracking-tight">单日工时与工作交付详情</h3>
                  <p className="text-xs text-slate-500 font-medium">
                    {selectedEntry.project_name} · {selectedEntry.date_str} {selectedEntry.time_range !== "全天标准" ? `(${selectedEntry.time_range})` : ""}
                  </p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span
                  className={`px-2.5 py-1 rounded-full text-[11px] font-bold border ${
                    selectedEntry.is_invoiced
                      ? "bg-emerald-50 text-emerald-700 border-emerald-200/80"
                      : "bg-amber-50 text-amber-700 border-amber-200/80"
                  }`}
                >
                  {selectedEntry.is_invoiced ? "✓ 已结清" : "⏳ 待结清"}
                </span>
                <button
                  onClick={() => setShowDetailModal(false)}
                  className="w-7 h-7 flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-100 rounded-lg text-sm font-bold transition-colors"
                >
                  ✕
                </button>
              </div>
            </div>

            {/* 核算指标网格卡片 */}
            <div className={`grid gap-3 p-3.5 bg-slate-50/80 border border-slate-200/80 rounded-xl ${hideAmount ? "grid-cols-2" : "grid-cols-3"}`}>
              <div>
                <span className="text-[11px] font-semibold text-slate-400 block mb-0.5">核算工时</span>
                <div className="text-sm font-extrabold text-slate-900 font-mono flex items-baseline gap-1">
                  {selectedEntry.duration_text}
                </div>
              </div>
              <div>
                <span className="text-[11px] font-semibold text-slate-400 block mb-0.5">折算人天</span>
                <div className="text-sm font-extrabold text-indigo-600 font-mono">
                  {selectedEntry.days_text}
                </div>
              </div>
              {!hideAmount && (
                <div>
                  <span className="text-[11px] font-semibold text-slate-400 block mb-0.5">应收结算金额</span>
                  <div className="text-sm font-extrabold text-emerald-600 font-mono">
                    {selectedEntry.amount_text}
                  </div>
                </div>
              )}
            </div>

            {/* 录音/凭证组件 */}
            {selectedEntry.has_audio && (
              <div className="p-3 bg-indigo-50/60 border border-indigo-100/80 rounded-xl flex items-center justify-between">
                <span className="text-xs font-semibold text-indigo-900 flex items-center gap-2">
                  <Volume2 className="w-4 h-4 text-indigo-600" />
                  已留存当日会议录音/工时语音凭证
                </span>
                <button
                  onClick={() => invoke("play_audio", { audioPath: selectedEntry.audio_path })}
                  className="px-3 py-1 bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg text-xs font-bold shadow-xs transition-colors flex items-center gap-1"
                >
                  ▶ 播放录音
                </button>
              </div>
            )}

            {/* 结构化工作内容与交付清单 */}
            <div className="space-y-2">
              <div className="flex items-center justify-between">
                <span className="text-xs font-bold text-slate-700 flex items-center gap-1.5">
                  <span>💼</span> 交付工作内容清单
                </span>
                <span className="text-[11px] text-slate-400 font-medium">按单项去重归集</span>
              </div>

              <div className="p-3.5 rounded-xl bg-slate-50 border border-slate-200/80 max-h-60 overflow-y-auto space-y-2">
                {(() => {
                  // 智能清洗与分项切分：支持分号、句号、换行，并剥除系统算法标签
                  const cleanText = (selectedEntry.summary_text || "")
                    .replace(/\[削峰回填\]\s*/g, "")
                    .replace(/\[超额顺延\]\s*/g, "");
                  
                  const items = cleanText
                    .split(/[；;。\n]+/)
                    .map((s) => s.trim())
                    .filter((s) => s.length > 0);

                  if (items.length === 0) {
                    return <div className="text-xs text-slate-400 italic py-2 text-center">暂无详细工作纪要描述</div>;
                  }

                  return items.map((item, idx) => (
                    <div
                      key={idx}
                      className="p-2.5 bg-white border border-slate-200/70 rounded-lg text-xs text-slate-800 leading-relaxed flex items-start gap-2 shadow-2xs hover:border-indigo-200 transition-colors"
                    >
                      <span className="w-4 h-4 rounded-full bg-slate-100 text-slate-500 font-bold text-[10px] flex items-center justify-center shrink-0 mt-0.5">
                        {idx + 1}
                      </span>
                      <span className="flex-1 font-medium">{item}</span>
                    </div>
                  ));
                })()}
              </div>
            </div>

            {/* 底部按钮栏 */}
            <div className="flex items-center justify-between pt-2 border-t border-slate-100">
              <button
                onClick={() => {
                  const cleanText = (selectedEntry.summary_text || "")
                    .replace(/\[削峰回填\]\s*/g, "")
                    .replace(/\[超额顺延\]\s*/g, "");
                  navigator.clipboard.writeText(cleanText);
                  showToast("交付清单已整理并复制到剪贴板！");
                }}
                className="px-3.5 py-2 bg-indigo-50 hover:bg-indigo-100 text-indigo-700 text-xs font-bold rounded-lg border border-indigo-200/60 flex items-center gap-1.5 transition-colors cursor-pointer"
              >
                <Copy className="w-3.5 h-3.5" />
                复制交付清单
              </button>
              
              <button
                onClick={() => setShowDetailModal(false)}
                className="px-5 py-2 bg-slate-100 hover:bg-slate-200 text-slate-700 font-bold text-xs rounded-lg transition-colors cursor-pointer"
              >
                关闭
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 4: 手动补录工时 ---------------- */}
      {showManualEntryModal && activeProject && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900 flex items-center gap-2">
                <PlusCircle className="w-4 h-4 text-indigo-600" />
                补录工时投入
              </h3>
              <button
                onClick={() => setShowManualEntryModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">归属项目</label>
                <div className="p-2.5 rounded-lg bg-slate-50 border border-slate-200 text-xs font-bold text-slate-800">
                  {activeProject.name} <span className="font-normal text-slate-500">({activeProject.client})</span>
                </div>
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">工作日期</label>
                <input
                  type="date"
                  value={manualEntryDate}
                  onChange={(e) => setManualEntryDate(e.target.value)}
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="text-xs font-bold text-slate-700 block mb-1">开始时间</label>
                  <input
                    type="time"
                    value={manualEntryStartTime}
                    onChange={(e) => setManualEntryStartTime(e.target.value)}
                    className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                  />
                </div>
                <div>
                  <label className="text-xs font-bold text-slate-700 block mb-1">结束时间</label>
                  <input
                    type="time"
                    value={manualEntryEndTime}
                    onChange={(e) => setManualEntryEndTime(e.target.value)}
                    className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                  />
                </div>
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">工作内容 / 交付说明</label>
                <textarea
                  value={manualEntrySummary}
                  onChange={(e) => setManualEntrySummary(e.target.value)}
                  placeholder="例如：补充完成多租户认证模块的逻辑测试与排查"
                  rows={3}
                  className="w-full p-2.5 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                />
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowManualEntryModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleLogManualEntry}
                className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm transition-all"
              >
                确认补录工时
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 5: 补录会议与上传录音 ---------------- */}
      {showManualMeetingModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900 flex items-center gap-2">
                <Upload className="w-4 h-4 text-indigo-600" />
                补录会议与录音凭证
              </h3>
              <button
                onClick={() => setShowManualMeetingModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">会议议题</label>
                <input
                  type="text"
                  value={manualMeetingTopic}
                  onChange={(e) => setManualMeetingTopic(e.target.value)}
                  placeholder="例如：跨部门对接评审会、业务架构宣讲"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">归属项目</label>
                <select
                  value={manualMeetingPid}
                  onChange={(e) => setManualMeetingPid(e.target.value)}
                  className="w-full h-9 px-2 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                >
                  <option value="">⚡ 独立临时会议 (单独立项)</option>
                  {projects.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name} ({p.client})
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">会议日期</label>
                <input
                  type="date"
                  value={manualMeetingDate}
                  onChange={(e) => setManualMeetingDate(e.target.value)}
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-1 focus:ring-indigo-500"
                />
              </div>

              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="text-xs font-bold text-slate-700 block mb-1">开始时间</label>
                  <input
                    type="time"
                    value={manualMeetingStartTime}
                    onChange={(e) => setManualMeetingStartTime(e.target.value)}
                    className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                  />
                </div>
                <div>
                  <label className="text-xs font-bold text-slate-700 block mb-1">结束时间</label>
                  <input
                    type="time"
                    value={manualMeetingEndTime}
                    onChange={(e) => setManualMeetingEndTime(e.target.value)}
                    className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                  />
                </div>
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">会议费率 (元/小时)</label>
                <input
                  type="number"
                  value={manualMeetingRate}
                  onChange={(e) => setManualMeetingRate(e.target.value)}
                  placeholder="350"
                  className="w-full h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 focus:outline-none"
                />
              </div>

              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">本地已有录音文件路径 (支持 .wav / .mp3 / .m4a)</label>
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    value={manualMeetingAudioPath}
                    onChange={(e) => setManualMeetingAudioPath(e.target.value)}
                    placeholder="/home/beatle/meeting.wav 或留空"
                    className="flex-1 h-9 px-3 rounded-lg border border-slate-200 text-xs text-slate-800 font-mono focus:outline-none"
                  />
                  <input
                    type="file"
                    id="audioFileInput"
                    accept="audio/*,.wav,.mp3,.m4a"
                    className="hidden"
                    onChange={(e) => {
                      const file = e.target.files?.[0];
                      if (file) {
                        // 在现代浏览器中无法直接读取完整物理路径，支持由用户确认或输入
                        const p = (file as any).path || file.name;
                        setManualMeetingAudioPath(p);
                      }
                    }}
                  />
                  <label
                    htmlFor="audioFileInput"
                    className="h-9 px-3 bg-slate-100 hover:bg-slate-200 border border-slate-200 rounded-lg text-xs font-bold text-slate-700 flex items-center cursor-pointer transition-colors"
                  >
                    浏览
                  </label>
                </div>
                <p className="text-[10px] text-slate-400 mt-1">
                  💡 录音文件将被安全归档至 MythManDay 数据目录，并在补登完成后支持一键播放与 AI 纪要转写。
                </p>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowManualMeetingModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                onClick={handleLogManualMeeting}
                className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm transition-all"
              >
                保存并归档会议
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 6: 快捷工作便签弹窗 ---------------- */}
      {showNoteModal && activeProject && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50">
          <div className="w-full max-w-md bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-slate-100">
              <h3 className="text-base font-extrabold text-slate-900 flex items-center gap-2">
                <StickyNote className="w-4 h-4 text-amber-500" />
                记便签 · {activeProject.name}
              </h3>
              <button
                onClick={() => setShowNoteModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-3">
              <div>
                <label className="text-xs font-bold text-slate-700 block mb-1">
                  输入工作随笔、需求变更或沟通速记
                </label>
                <textarea
                  value={noteInput}
                  onChange={(e) => setNoteInput(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                      handleAddNote();
                      setShowNoteModal(false);
                    }
                  }}
                  placeholder="例如：客户提到结算模块下周三前必须支持按月开票导出；稍后跟进修复验证。"
                  rows={4}
                  className="w-full p-3 rounded-xl bg-slate-50 border border-slate-200 text-xs text-slate-800 focus:outline-none focus:ring-2 focus:ring-indigo-500/20 focus:border-indigo-500 leading-relaxed"
                  autoFocus
                />
                <p className="text-[10px] text-slate-400 mt-1">快捷键: 按 Ctrl+Enter 可直接提交保存</p>
              </div>

              {/* 历史便签快速预览 */}
              {notes.length > 0 && (
                <div className="pt-2 border-t border-slate-100 space-y-2">
                  <div className="text-[11px] font-bold text-slate-500">已存备忘（{notes.length} 条）</div>
                  <div className="max-h-36 overflow-y-auto space-y-1.5 pr-1">
                    {notes.slice(0, 5).map((n) => (
                      <div
                        key={n.id}
                        className="p-2 rounded-lg bg-slate-50 border border-slate-200/60 flex items-center justify-between text-[11px]"
                      >
                        <span className="text-slate-700 truncate mr-2">
                          <span className="text-indigo-600 font-mono mr-1">[{n.time_text}]</span>
                          {n.content}
                        </span>
                        <button
                          onClick={() => handleDeleteNote(n.id)}
                          className="text-slate-400 hover:text-rose-500 shrink-0 p-0.5"
                          title="删除"
                        >
                          <Trash2 className="w-3 h-3" />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowNoteModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                关闭
              </button>
              <button
                onClick={async () => {
                  if (noteInput.trim()) {
                    await handleAddNote();
                    setShowNoteModal(false);
                  }
                }}
                className="px-4 py-2 text-xs font-bold bg-amber-500 hover:bg-amber-600 text-white rounded-lg shadow-sm transition-all"
              >
                保存便签
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 6: 高级多维筛选弹窗 ---------------- */}
      {showFilterModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-100">
          <div className="w-full max-w-lg bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-5">
            <div className="flex items-center justify-between pb-3 border-b border-slate-100">
              <h3 className="text-sm font-extrabold text-slate-900 flex items-center gap-2">
                <Filter className="w-4 h-4 text-indigo-600" />
                多维对账条件筛选
              </h3>
              <button
                onClick={() => setShowFilterModal(false)}
                className="text-slate-400 hover:text-slate-700 text-sm font-bold"
              >
                ✕
              </button>
            </div>

            <div className="space-y-4">
              {/* 跨月份多选 */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-bold text-slate-700">核算月份（支持跨多月）</label>
                  {reportSelectedMonths.length > 0 && (
                    <button
                      onClick={() => setReportSelectedMonths([])}
                      className="text-[11px] text-indigo-600 hover:underline"
                    >
                      清空月份 (全选历史)
                    </button>
                  )}
                </div>
                <div className="flex flex-wrap gap-1.5 max-h-28 overflow-y-auto p-2 bg-slate-50 border border-slate-200 rounded-xl">
                  {(() => {
                    const monthsList: string[] = [];
                    const d = new Date();
                    for (let i = 0; i < 12; i++) {
                      const ym = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
                      monthsList.push(ym);
                      d.setMonth(d.getMonth() - 1);
                    }
                    return monthsList.map((m) => {
                      const isChecked = reportSelectedMonths.includes(m);
                      return (
                        <button
                          key={m}
                          onClick={() => {
                            const updated = isChecked
                              ? reportSelectedMonths.filter((x) => x !== m)
                              : [...reportSelectedMonths, m];
                            setReportSelectedMonths(updated);
                          }}
                          className={`px-2.5 py-1 rounded-lg text-xs font-mono transition-all ${
                            isChecked
                              ? "bg-indigo-600 text-white font-bold shadow-2xs"
                              : "bg-white text-slate-700 border border-slate-200 hover:bg-slate-100"
                          }`}
                        >
                          {m}
                        </button>
                      );
                    });
                  })()}
                </div>
                <p className="text-[10px] text-slate-400">未选择任何月份时，自动计算全部历史发生工时</p>
              </div>

              {/* 跨项目多选 */}
              <div className="space-y-1.5">
                <div className="flex items-center justify-between">
                  <label className="text-xs font-bold text-slate-700">归属项目（支持跨项目）</label>
                  {reportSelectedProjects.length > 0 && (
                    <button
                      onClick={() => setReportSelectedProjects([])}
                      className="text-[11px] text-indigo-600 hover:underline"
                    >
                      包含全部项目
                    </button>
                  )}
                </div>
                <div className="flex flex-wrap gap-1.5 max-h-32 overflow-y-auto p-2 bg-slate-50 border border-slate-200 rounded-xl">
                  {projects.map((p) => {
                    const isChecked = reportSelectedProjects.includes(p.id);
                    return (
                      <button
                        key={p.id}
                        onClick={() => {
                          const updated = isChecked
                            ? reportSelectedProjects.filter((x) => x !== p.id)
                            : [...reportSelectedProjects, p.id];
                          setReportSelectedProjects(updated);
                        }}
                        className={`px-2.5 py-1 rounded-lg text-xs transition-all truncate max-w-[160px] ${
                          isChecked
                            ? "bg-indigo-600 text-white font-bold shadow-2xs"
                            : "bg-white text-slate-700 border border-slate-200 hover:bg-slate-100"
                        }`}
                        title={p.name}
                      >
                        {p.name}
                      </button>
                    );
                  })}
                </div>
                <p className="text-[10px] text-slate-400">未选择项目时，自动聚合全部已立项项目流水</p>
              </div>
            </div>

            <div className="flex items-center justify-end gap-2 pt-3 border-t border-slate-100">
              <button
                onClick={() => setShowFilterModal(false)}
                className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
              >
                取消
              </button>
              <button
                onClick={() => {
                  fetchSettlementReport();
                  setShowFilterModal(false);
                  showToast("筛选条件已更新！");
                }}
                className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm transition-all"
              >
                应用筛选
              </button>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 7: 对账报表预览与导出弹窗 ---------------- */}
      {showReportExportModal && (
        <div className="fixed inset-0 bg-slate-900/40 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-100">
          <div className="w-full max-w-3xl bg-white rounded-2xl border border-slate-200 shadow-2xl p-6 space-y-4 max-h-[85vh] flex flex-col">
            <div className="flex items-center justify-between pb-3 border-b border-slate-100 shrink-0">
              <div className="flex items-center gap-2">
                <FileSpreadsheet className="w-4 h-4 text-indigo-600" />
                <h3 className="text-sm font-extrabold text-slate-900">
                  多项目人天结算对账单预览
                </h3>
              </div>
              <div className="flex items-center gap-2">
                {/* 视图切换 */}
                <div className="flex items-center bg-slate-100 rounded-lg p-0.5 border border-slate-200/60 text-xs">
                  <button
                    onClick={() => setReportViewMode("preview")}
                    className={`px-2.5 py-1 rounded-md transition-all ${
                      reportViewMode === "preview" ? "bg-white text-slate-900 font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    排版预览
                  </button>
                  <button
                    onClick={() => setReportViewMode("raw")}
                    className={`px-2.5 py-1 rounded-md transition-all ${
                      reportViewMode === "raw" ? "bg-white text-slate-900 font-bold shadow-2xs" : "text-slate-500 hover:text-slate-800"
                    }`}
                  >
                    Markdown 源码
                  </button>
                </div>

                <button
                  onClick={() => setShowReportExportModal(false)}
                  className="text-slate-400 hover:text-slate-700 text-sm font-bold ml-2"
                >
                  ✕
                </button>
              </div>
            </div>

            {/* 报表内容 */}
            <div className="flex-1 overflow-y-auto">
              {reportViewMode === "preview" ? (
                <div
                  className="p-5 rounded-xl bg-slate-50 border border-slate-200 text-xs markdown-body"
                  dangerouslySetInnerHTML={{
                    __html: renderMarkdown(settlementReport?.markdown || "正在聚合对账单..."),
                  }}
                />
              ) : (
                <div className="p-4 rounded-xl bg-slate-50 border border-slate-200 font-mono text-xs text-slate-800 whitespace-pre-wrap leading-relaxed">
                  {settlementReport?.markdown || "正在聚合对账单..."}
                </div>
              )}
            </div>

            {/* 底部操作区 */}
            <div className="flex items-center justify-between pt-3 border-t border-slate-100 shrink-0">
              <span className="text-[11px] text-slate-400">
                可直接复制文本发给客户、财务或作为项目周报/月报交付凭据
              </span>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowReportExportModal(false)}
                  className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors"
                >
                  关闭
                </button>
                <button
                  onClick={() => {
                    setShowReportExportModal(false);
                    setExportSelectedProjects(reportSelectedProjects.length > 0 ? reportSelectedProjects : projects.map(p => p.id));
                    setExportSelectedMonths(reportSelectedMonths);
                    setShowExportSettingsModal(true);
                  }}
                  className="px-4 py-2 text-xs font-bold bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg shadow-sm flex items-center gap-1.5 transition-all cursor-pointer"
                >
                  <Download className="w-3.5 h-3.5" />
                  导出 Excel (.xlsx)
                </button>
                <button
                  onClick={() => {
                    if (settlementReport?.markdown) {
                      navigator.clipboard.writeText(settlementReport.markdown);
                      showToast("Markdown 结算对账单已复制到剪贴板！");
                    }
                  }}
                  className="px-4 py-2 text-xs font-bold bg-indigo-600 hover:bg-indigo-700 text-white rounded-lg shadow-sm flex items-center gap-1.5 transition-all"
                >
                  <Copy className="w-3.5 h-3.5" />
                  复制 Markdown
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 模态框 8: 专业 Excel 结算对账单导出配置 ---------------- */}
      {showExportSettingsModal && (
        <div className="fixed inset-0 bg-slate-900/50 backdrop-blur-xs flex items-center justify-center p-4 z-50 animate-in fade-in duration-150">
          <div className="w-full max-w-2xl bg-white rounded-2xl border border-slate-200/90 shadow-2xl p-6 space-y-5">
            {/* 标题 */}
            <div className="flex items-center justify-between pb-3 border-b border-slate-100">
              <div className="flex items-center gap-2.5">
                <div className="w-9 h-9 rounded-xl bg-emerald-50 border border-emerald-200/80 flex items-center justify-center text-emerald-600 text-lg font-bold">
                  📥
                </div>
                <div>
                  <h3 className="text-base font-extrabold text-slate-900 tracking-tight">
                    导出项目人天对账单 (Excel)
                  </h3>
                  <p className="text-xs text-slate-500 font-medium">
                    每个项目生成独立 .xlsx 文件 · 各月份作为独立工作表 (Sheet)
                  </p>
                </div>
              </div>
              <button
                onClick={() => setShowExportSettingsModal(false)}
                className="w-7 h-7 flex items-center justify-center text-slate-400 hover:text-slate-700 hover:bg-slate-100 rounded-lg text-sm font-bold transition-colors cursor-pointer"
              >
                ✕
              </button>
            </div>

            {/* 1. 核算结算策略选择 (核心单选卡片) */}
            <div className="space-y-2">
              <label className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                <span>⚖️</span> 选择人天核算与流动策略 (必选)
              </label>
              <div className="grid grid-cols-2 gap-2.5">
                {/* 策略 1: 削峰填谷 */}
                <div
                  onClick={() => setExportStrategy("rebalance")}
                  className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                    exportStrategy === "rebalance"
                      ? "bg-indigo-50/60 border-indigo-500 ring-2 ring-indigo-500/20"
                      : "bg-white hover:bg-slate-50 border-slate-200/80"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-extrabold text-slate-900 flex items-center gap-1.5">
                      <span>🛡️</span> 合规削峰填谷
                    </span>
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-indigo-100 text-indigo-700 font-bold">标准推荐</span>
                  </div>
                  <p className="text-[11px] text-slate-500 leading-relaxed">
                    工作日单日封顶 8 小时 (1.0天)；溢出工时回填历史不足工作日或顺延未来工作日，周末不排工时。
                  </p>
                </div>

                {/* 策略 2: 保底 8 小时 */}
                <div
                  onClick={() => setExportStrategy("min8_carry")}
                  className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                    exportStrategy === "min8_carry"
                      ? "bg-indigo-50/60 border-indigo-500 ring-2 ring-indigo-500/20"
                      : "bg-white hover:bg-slate-50 border-slate-200/80"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-extrabold text-slate-900 flex items-center gap-1.5">
                      <span>💎</span> 不足8h按8h (超额顺延)
                    </span>
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-amber-100 text-amber-800 font-bold">顾问/驻场</span>
                  </div>
                  <p className="text-[11px] text-slate-500 leading-relaxed">
                    只要当天有效投入，基准按 1.0 人天起步保底；超过 8 小时部分自动结转为新工作日追加。
                  </p>
                </div>

                {/* 策略 3: 单日封顶 8h，超额直接顺延新天 */}
                <div
                  onClick={() => setExportStrategy("cap8_carry")}
                  className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                    exportStrategy === "cap8_carry"
                      ? "bg-indigo-50/60 border-indigo-500 ring-2 ring-indigo-500/20"
                      : "bg-white hover:bg-slate-50 border-slate-200/80"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-extrabold text-slate-900 flex items-center gap-1.5">
                      <span>⏩</span> 封顶顺延新天
                    </span>
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-slate-100 text-slate-600 font-bold">按日延期</span>
                  </div>
                  <p className="text-[11px] text-slate-500 leading-relaxed">
                    当天实报实销但不超过 8 小时；超出 8 小时的溢出工时直接结转出新的工作日追加在后。
                  </p>
                </div>

                {/* 策略 4: 严格实际工时合并 */}
                <div
                  onClick={() => setExportStrategy("exact")}
                  className={`p-3 rounded-xl border text-left cursor-pointer transition-all ${
                    exportStrategy === "exact"
                      ? "bg-indigo-50/60 border-indigo-500 ring-2 ring-indigo-500/20"
                      : "bg-white hover:bg-slate-50 border-slate-200/80"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1">
                    <span className="text-xs font-extrabold text-slate-900 flex items-center gap-1.5">
                      <span>⏱️</span> 实际物理工时合并
                    </span>
                    <span className="text-[10px] px-1.5 py-0.2 rounded bg-slate-100 text-slate-600 font-bold">实报实销</span>
                  </div>
                  <p className="text-[11px] text-slate-500 leading-relaxed">
                    同一天同项目多次打卡合并为唯一一行，按当天实际物理投入比例换算人天，不顺延、不平摊。
                  </p>
                </div>
              </div>
            </div>

            {/* 2. 项目勾选多选 (每个项目一个独立文件) */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <label className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                  <span>📁</span> 选择要导出的项目 (每个勾选项将生成一个独立 .xlsx 文件)
                </label>
                <div className="flex items-center gap-2">
                  <button
                    onClick={() => setExportSelectedProjects(projects.map(p => p.id))}
                    className="text-[11px] text-indigo-600 hover:underline font-bold cursor-pointer"
                  >
                    全选
                  </button>
                  <span className="text-slate-300">|</span>
                  <button
                    onClick={() => setExportSelectedProjects([])}
                    className="text-[11px] text-slate-400 hover:text-slate-600 font-medium cursor-pointer"
                  >
                    清空
                  </button>
                </div>
              </div>
              <div className="max-h-32 overflow-y-auto p-2 bg-slate-50 border border-slate-200/80 rounded-xl grid grid-cols-2 gap-2">
                {projects.map((p) => {
                  const isChecked = exportSelectedProjects.includes(p.id);
                  return (
                    <label
                      key={p.id}
                      className={`flex items-center gap-2 p-2 rounded-lg border text-xs cursor-pointer transition-all ${
                        isChecked
                          ? "bg-white border-indigo-300 text-slate-900 shadow-2xs font-semibold"
                          : "border-transparent text-slate-600 hover:bg-slate-100/70"
                      }`}
                    >
                      <input
                        type="checkbox"
                        checked={isChecked}
                        onChange={(e) => {
                          if (e.target.checked) {
                            setExportSelectedProjects([...exportSelectedProjects, p.id]);
                          } else {
                            setExportSelectedProjects(exportSelectedProjects.filter((id) => id !== p.id));
                          }
                        }}
                        className="rounded text-indigo-600"
                      />
                      <span className="truncate flex-1">{p.name}</span>
                    </label>
                  );
                })}
              </div>
            </div>

            {/* 3. 月份勾选多选 (每个月份作为独立 Sheet) */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <label className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                  <span>📅</span> 选择导出的月份 (在项目文件内按月拆分 Sheet 工作表)
                </label>
              </div>
              <div className="flex flex-wrap gap-2">
                {[
                  currentMonthStr,
                  ...Array.from({ length: 5 }).map((_, i) => {
                    const d = new Date(now.getFullYear(), now.getMonth() - (i + 1), 1);
                    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
                  }),
                ].map((mStr) => {
                  const isChecked = exportSelectedMonths.includes(mStr);
                  const [y, m] = mStr.split("-");
                  return (
                    <button
                      key={mStr}
                      onClick={() => {
                        if (isChecked) {
                          if (exportSelectedMonths.length > 1) {
                            setExportSelectedMonths(exportSelectedMonths.filter((x) => x !== mStr));
                          } else {
                            showToast("至少需保留一个月份！");
                          }
                        } else {
                          setExportSelectedMonths([...exportSelectedMonths, mStr]);
                        }
                      }}
                      className={`px-3 py-1.5 rounded-lg text-xs font-bold border transition-all cursor-pointer ${
                        isChecked
                          ? "bg-indigo-600 text-white border-indigo-600 shadow-2xs"
                          : "bg-white text-slate-600 border-slate-200 hover:bg-slate-50"
                      }`}
                    >
                      {`${y}年${m}月`}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* 4. 导出字段与金额显示配置 (新增选项) */}
            <div className="p-3 bg-slate-50 border border-slate-200/80 rounded-xl flex items-center justify-between">
              <div>
                <span className="text-xs font-bold text-slate-800 flex items-center gap-1.5">
                  <span>💰</span> 导出包含结算金额与单价信息
                </span>
                <p className="text-[11px] text-slate-500 mt-0.5">
                  关闭后，Excel 中将直接剔除「核算金额」列及金额汇总，仅保留投入工时、人天折算与交付成果
                </p>
              </div>
              <label className="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  checked={exportIncludeAmount}
                  onChange={(e) => setExportIncludeAmount(e.target.checked)}
                  className="sr-only peer"
                />
                <div className="w-9 h-5 bg-slate-300 peer-focus:outline-none rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-slate-300 after:border after:rounded-full after:h-4 after:w-4 after:transition-all peer-checked:bg-emerald-600"></div>
                <span className="ml-2 text-xs font-bold text-slate-700">
                  {exportIncludeAmount ? "包含金额" : "隐藏金额"}
                </span>
              </label>
            </div>

            {/* 底部按钮栏 */}
            <div className="flex items-center justify-between pt-3 border-t border-slate-100">
              <div className="text-[11px] text-slate-500">
                将生成 <span className="font-bold text-slate-900">{exportSelectedProjects.length || projects.length}</span> 个独立文件，
                每个文件内含 <span className="font-bold text-slate-900">{exportSelectedMonths.length}</span> 个月份工作表
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setShowExportSettingsModal(false)}
                  className="px-4 py-2 text-xs font-bold text-slate-600 hover:bg-slate-100 rounded-lg transition-colors cursor-pointer"
                >
                  取消
                </button>
                <button
                  disabled={isExportingExcel}
                  onClick={executeExportExcel}
                  className="px-5 py-2 text-xs font-bold bg-emerald-600 hover:bg-emerald-700 text-white rounded-lg shadow-sm flex items-center gap-1.5 transition-all cursor-pointer disabled:opacity-50"
                >
                  <Download className="w-3.5 h-3.5" />
                  {isExportingExcel ? "正在生成..." : "确认导出"}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* ---------------- 底部轻量 Toast 提示 (带操作按钮) ---------------- */}
      {toast && (
        <div className="fixed bottom-6 left-1/2 -translate-x-1/2 bg-slate-900/95 text-white px-5 py-2.5 rounded-2xl text-xs font-bold shadow-2xl flex items-center gap-3 animate-in fade-in slide-in-from-bottom-2 duration-150 z-50 border border-slate-700/80">
          <CheckCircle2 className="w-4 h-4 text-emerald-400 shrink-0" />
          <span className="tracking-wide">{toast.message}</span>
          {toast.actionLabel && toast.onAction && (
            <button
              onClick={toast.onAction}
              className="ml-2 px-3 py-1 bg-emerald-500 hover:bg-emerald-600 text-slate-950 font-extrabold rounded-lg text-[11px] shadow-xs transition-colors cursor-pointer flex items-center gap-1 shrink-0"
            >
              {toast.actionLabel}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

export default App;
