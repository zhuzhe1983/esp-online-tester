# M5Stack Stamp-C5 外观与知识入库

ESP32-C5HF4 双频Wi-Fi6模块。模型来自官方KiCad封装内嵌STEP，保留真实USB开放曲面和已装配电子元件；RF屏蔽盖尚未包含。

来源：[官方产品文档](https://docs.m5stack.com/en/core/Stamp-C5)，M5_Hardware 固定 `e29c139cbc7c80b5ccc5557e5ab8025a7a19a109`。逐源 URL、UTC、SHA-256、下载状态见 [knowledge](../../../web/platform/knowledge/m5stack-s016.json) 与 [source audit](../evidence/m5stack-s016/source-audit.json)。旧抓取保留原始时间，不伪造刷新日期。

Native STEP solids, open shells and standalone faces in stable source order; single rigid centering/orientation transform without rescaling. Colors/names read from XCAF; missing values remain explicit unknown.

[model-plan.json](../../../web/platform/assets/m5stack-s016/model-plan.json) 记录组件选择、刚体变换、照片裁切、纹理投影与估算表面；厂家几何不缩放。STL 保留源三角形；STEP 使用 OpenCascade7.8.1.1 以0.05mm弦偏差/0.2rad角度细分。UNKNOWN 是中性占位，未编造 PCB 走线或未核实电子元件。

| 检查 | 实测或误差 mm | 目标 mm | 结果 |
| --- | ---: | ---: | --- |
| pcb-main source-fidelity | 4.7420434690081947e-07 | 0 | 通过 |
| cad-part-1 source-fidelity | 4.7082258447517233e-07 | 0 | 通过 |
| cad-part-2 source-fidelity | 4.7310423312296734e-07 | 0 | 通过 |
| cad-part-3 source-fidelity | 4.7673158931615944e-07 | 0 | 通过 |
| cad-part-4 source-fidelity | 2.383399912986306e-07 | 0 | 通过 |
| cad-part-5 source-fidelity | 2.3815224992063122e-07 | 0 | 通过 |
| cad-part-6 source-fidelity | 4.698589286533661e-07 | 0 | 通过 |
| cad-part-7 source-fidelity | 2.3021846118354006e-07 | 0 | 通过 |
| cad-part-8 source-fidelity | 2.3021846118354006e-07 | 0 | 通过 |
| cad-part-9 source-fidelity | 2.3792498282659835e-07 | 0 | 通过 |
| cad-part-10 source-fidelity | 2.2545119193750907e-07 | 0 | 通过 |
| cad-part-11 source-fidelity | 2.2545119193750907e-07 | 0 | 通过 |
| cad-part-12 source-fidelity | 2.381515820104596e-07 | 0 | 通过 |
| cad-part-13 source-fidelity | 2.2551216360966464e-07 | 0 | 通过 |
| cad-part-14 source-fidelity | 2.2551216360966464e-07 | 0 | 通过 |
| cad-part-15 source-fidelity | 2.3838984297697152e-07 | 0 | 通过 |
| cad-part-16 source-fidelity | 2.2551216360966464e-07 | 0 | 通过 |
| cad-part-17 source-fidelity | 2.2551216360966464e-07 | 0 | 通过 |
| cad-part-18 source-fidelity | 2.3838984297697152e-07 | 0 | 通过 |
| cad-part-19 source-fidelity | 2.383076429524067e-07 | 0 | 通过 |
| cad-part-20 source-fidelity | 2.3694761885906246e-07 | 0 | 通过 |
| cad-part-21 source-fidelity | 2.3640446222827904e-07 | 0 | 通过 |
| cad-part-22 source-fidelity | 2.3777731072982533e-07 | 0 | 通过 |
| cad-part-23 source-fidelity | 4.768104631125425e-07 | 0 | 通过 |
| cad-part-24 source-fidelity | 4.7674459224822385e-07 | 0 | 通过 |
| cad-part-25 source-fidelity | 4.7644491729670335e-07 | 0 | 通过 |
| cad-part-26 source-fidelity | 2.3813945304596018e-07 | 0 | 通过 |
| cad-part-27 source-fidelity | 2.3826874695487277e-07 | 0 | 通过 |
| cad-part-28 source-fidelity | 2.384037509628456e-07 | 0 | 通过 |
| cad-part-29 source-fidelity | 2.3021846118354006e-07 | 0 | 通过 |
| cad-part-30 source-fidelity | 2.3021846118354006e-07 | 0 | 通过 |
| cad-part-31 source-fidelity | 2.3792498282659835e-07 | 0 | 通过 |
| cad-part-32 source-fidelity | 2.383076429524067e-07 | 0 | 通过 |
| cad-part-33 source-fidelity | 1.2528482962181897e-07 | 0 | 通过 |
| cad-part-34 source-fidelity | 1.2528492376873146e-07 | 0 | 通过 |
| cad-part-35 source-fidelity | 1.2528482962181897e-07 | 0 | 通过 |
| cad-part-36 source-fidelity | 1.2528492376873146e-07 | 0 | 通过 |
| cad-part-37 source-fidelity | 1.6582785544727585e-07 | 0 | 通过 |
| cad-part-38 source-fidelity | 2.3806599536158046e-07 | 0 | 通过 |
| cad-part-39 source-fidelity | 2.2438649160960722e-07 | 0 | 通过 |
| cad-part-40 source-fidelity | 2.3806599536158046e-07 | 0 | 通过 |
| cad-part-41 source-fidelity | 6.246327632908333e-08 | 0 | 通过 |
| cad-part-42 source-fidelity | 6.214170866769564e-08 | 0 | 通过 |
| cad-part-43 source-fidelity | 6.160474974592489e-08 | 0 | 通过 |
| cad-part-44 source-fidelity | 6.11934947158943e-08 | 0 | 通过 |
| cad-part-45 source-fidelity | 6.07013790343558e-08 | 0 | 通过 |
| cad-part-46 source-fidelity | 6.246327632908333e-08 | 0 | 通过 |
| cad-part-47 source-fidelity | 6.209107805688063e-08 | 0 | 通过 |
| cad-part-48 source-fidelity | 6.167027688519511e-08 | 0 | 通过 |
| cad-part-49 source-fidelity | 6.13604260735201e-08 | 0 | 通过 |
| cad-part-50 source-fidelity | 6.089608550752246e-08 | 0 | 通过 |
| cad-part-51 source-fidelity | 6.048483047749187e-08 | 0 | 通过 |
| cad-part-52 source-fidelity | 5.999271479595336e-08 | 0 | 通过 |
| cad-part-53 source-fidelity | 6.549366915464816e-08 | 0 | 通过 |
| cad-part-54 source-fidelity | 2.3694680706398685e-07 | 0 | 通过 |
| cad-part-55 source-fidelity | 2.3640365043320344e-07 | 0 | 通过 |
| cad-part-56 source-fidelity | 2.3777812252490094e-07 | 0 | 通过 |
| cad-part-57 source-fidelity | 2.379931167695304e-07 | 0 | 通过 |
| cad-part-58 source-fidelity | 2.3560467354855064e-07 | 0 | 通过 |
| cad-part-59 source-fidelity | 2.38180048128811e-07 | 0 | 通过 |
| cad-part-60 source-fidelity | 4.725404707528469e-07 | 0 | 通过 |
| cad-part-61 source-fidelity | 5.630666644051985e-08 | 0 | 通过 |
| cad-part-62 source-fidelity | 1.168086551928127e-07 | 0 | 通过 |
| cad-part-63 source-fidelity | 5.7195138403187684e-08 | 0 | 通过 |
| cad-part-64 source-fidelity | 2.9772763365798482e-08 | 0 | 通过 |
| cad-part-65 source-fidelity | 5.89146957885589e-08 | 0 | 通过 |
| cad-part-66 source-fidelity | 5.944767567100939e-08 | 0 | 通过 |
| cad-part-67 source-fidelity | 5.757343535250925e-08 | 0 | 通过 |
| cad-part-68 source-fidelity | 5.757343535250925e-08 | 0 | 通过 |
| cad-part-69 source-fidelity | 1.1729313875719072e-07 | 0 | 通过 |
| cad-part-70 source-fidelity | 5.9244694483595595e-08 | 0 | 通过 |
| cad-part-71 source-fidelity | 1.1267464294917318e-07 | 0 | 通过 |
| cad-part-72 source-fidelity | 1.1596610782760308e-07 | 0 | 通过 |
| cad-part-73 source-fidelity | 1.1596610782760308e-07 | 0 | 通过 |
| cad-part-74 source-fidelity | 2.092798219166525e-07 | 0 | 通过 |
| cad-part-75 source-fidelity | 2.0450987037179402e-07 | 0 | 通过 |
| cad-part-76 source-fidelity | 2.1881972678272632e-07 | 0 | 通过 |
| cad-part-77 source-fidelity | 2.3128332937716323e-07 | 0 | 通过 |
| cad-part-78 source-fidelity | 2.2358968010394165e-07 | 0 | 通过 |
| cad-part-79 source-fidelity | 2.3544130911545835e-07 | 0 | 通过 |
| cad-part-80 source-fidelity | 2.2835963164880013e-07 | 0 | 通过 |
| cad-part-81 source-fidelity | 2.3530455628417712e-07 | 0 | 通过 |
| cad-part-82 source-fidelity | 2.305346029629618e-07 | 0 | 通过 |
| cad-part-83 source-fidelity | 2.1133353911295671e-07 | 0 | 通过 |
| cad-part-84 source-fidelity | 2.3544130911545835e-07 | 0 | 通过 |
| cad-part-85 source-fidelity | 2.3605328181020013e-07 | 0 | 通过 |
| cad-part-86 source-fidelity | 2.383988997323172e-07 | 0 | 通过 |
| cad-part-87 source-fidelity | 2.3362894818745872e-07 | 0 | 通过 |
| cad-part-88 source-fidelity | 1.168086551928127e-07 | 0 | 通过 |
| cad-part-89 source-fidelity | 2.288589957544218e-07 | 0 | 通过 |
| cad-part-90 source-fidelity | 1.157384703631692e-07 | 0 | 通过 |
| cad-part-91 source-fidelity | 2.3312958319365862e-07 | 0 | 通过 |
| cad-part-92 source-fidelity | 2.3312958319365862e-07 | 0 | 通过 |
| cad-part-93 source-fidelity | 2.1404977701422467e-07 | 0 | 通过 |
| cad-part-94 source-fidelity | 2.2358968010394165e-07 | 0 | 通过 |
| cad-part-95 source-fidelity | 2.0450987037179402e-07 | 0 | 通过 |
| cad-part-96 source-fidelity | 2.1881972678272632e-07 | 0 | 通过 |
| cad-part-97 source-fidelity | 2.092798219166525e-07 | 0 | 通过 |
| cad-part-98 source-fidelity | 2.0689283886099474e-07 | 0 | 通过 |
| cad-part-99 source-fidelity | 1.1676683975281321e-07 | 0 | 通过 |
| cad-part-100 source-fidelity | 2.8722796585611832e-08 | 0 | 通过 |
| cad-part-101 source-fidelity | 5.499166100264574e-08 | 0 | 通过 |
| cad-part-102 source-fidelity | 5.9244694483595595e-08 | 0 | 通过 |
| cad-part-103 source-fidelity | 2.9750782282178534e-08 | 0 | 通过 |
| cad-part-104 source-fidelity | 2.9544789281743533e-08 | 0 | 通过 |
| cad-part-105 source-fidelity | 2.9750782282178534e-08 | 0 | 通过 |
| cad-part-106 source-fidelity | 5.89146957885589e-08 | 0 | 通过 |
| cad-part-107 source-fidelity | 1.0012622553290385e-07 | 0 | 通过 |
| cad-part-108 source-fidelity | 5.8758179211082506e-08 | 0 | 通过 |
| cad-part-109 source-fidelity | 5.7195138403187684e-08 | 0 | 通过 |
| cad-part-110 source-fidelity | 5.8758179211082506e-08 | 0 | 通过 |
| cad-part-111 source-fidelity | 1.168086551928127e-07 | 0 | 通过 |
| cad-part-112 source-fidelity | 5.630666644051985e-08 | 0 | 通过 |
| cad-part-113 source-fidelity | 2.288589957544218e-07 | 0 | 通过 |
| cad-part-114 source-fidelity | 1.168086551928127e-07 | 0 | 通过 |
| cad-part-115 source-fidelity | 2.2996335147240643e-07 | 0 | 通过 |
| cad-part-116 source-fidelity | 1.157384703631692e-07 | 0 | 通过 |
| cad-part-117 source-fidelity | 2.3605328181020013e-07 | 0 | 通过 |
| cad-part-118 source-fidelity | 2.383988997323172e-07 | 0 | 通过 |
| cad-part-119 source-fidelity | 2.3472499677268388e-07 | 0 | 通过 |
| cad-part-120 source-fidelity | 2.305346029629618e-07 | 0 | 通过 |
| cad-part-121 source-fidelity | 2.9772763365798482e-08 | 0 | 通过 |
| cad-part-122 source-fidelity | 1.0989843479691785e-07 | 0 | 通过 |
| cad-part-123 source-fidelity | 2.1931909177652642e-07 | 0 | 通过 |
| cad-part-124 source-fidelity | 2.1697347385440935e-07 | 0 | 通过 |
| cad-part-125 source-fidelity | 2.3362894818745872e-07 | 0 | 通过 |
| cad-part-126 source-fidelity | 1.0989843479691785e-07 | 0 | 通过 |
| cad-part-127 source-fidelity | 2.2651337783230474e-07 | 0 | 通过 |
| cad-part-128 source-fidelity | 5.499166100264574e-08 | 0 | 通过 |
| cad-part-129 source-fidelity | 5.9277896369280825e-08 | 0 | 通过 |
| cad-part-130 source-fidelity | 1.9306115639494692e-07 | 0 | 通过 |
| cad-part-131 source-fidelity | 2.1404977701422467e-07 | 0 | 通过 |
| cad-part-132 source-fidelity | 2.3789953829123078e-07 | 0 | 通过 |
| cad-part-133 source-fidelity | 2.3472499677268388e-07 | 0 | 通过 |
| cad-part-134 source-fidelity | 2.2996335147240643e-07 | 0 | 通过 |
| cad-part-135 source-fidelity | 1.1267464294917318e-07 | 0 | 通过 |
| cad-part-136 source-fidelity | 1.1729313875719072e-07 | 0 | 通过 |
| cad-part-137 source-fidelity | 2.2408904243320649e-07 | 0 | 通过 |
| cad-part-138 source-fidelity | 2.1697347385440935e-07 | 0 | 通过 |
| cad-part-139 source-fidelity | 2.2651337783230474e-07 | 0 | 通过 |
| cad-part-140 source-fidelity | 2.3128332937716323e-07 | 0 | 通过 |
| cad-part-141 source-fidelity | 2.2408904243320649e-07 | 0 | 通过 |
| cad-part-142 source-fidelity | 2.1931909177652642e-07 | 0 | 通过 |
| cad-part-143 source-fidelity | 2.8722796585611832e-08 | 0 | 通过 |
| cad-part-144 source-fidelity | 2.9544789281743533e-08 | 0 | 通过 |
| cad-part-145 source-fidelity | 5.944767567100939e-08 | 0 | 通过 |
| cad-part-146 source-fidelity | 5.9277896369280825e-08 | 0 | 通过 |
| cad-part-147 source-fidelity | 2.3789953829123078e-07 | 0 | 通过 |
| cad-part-148 source-fidelity | 2.3530455628417712e-07 | 0 | 通过 |
| cad-part-149 source-fidelity | 2.2835963164880013e-07 | 0 | 通过 |
| cad-part-150 source-fidelity | 2.0689283886099474e-07 | 0 | 通过 |
| cad-part-151 source-fidelity | 1.1676683975281321e-07 | 0 | 通过 |
| cad-part-152 source-fidelity | 1.9306115639494692e-07 | 0 | 通过 |
| cad-part-153 source-fidelity | 2.1133353911295671e-07 | 0 | 通过 |
| cad-part-154 source-fidelity | 1.0012622553290385e-07 | 0 | 通过 |
| cad-part-155 source-fidelity | 2.2174342451108942e-07 | 0 | 通过 |
| cad-part-156 source-fidelity | 2.2174342451108942e-07 | 0 | 通过 |
| cad-part-157 source-fidelity | 2.383988997323172e-07 | 0 | 通过 |
| cad-part-158 source-fidelity | 2.350569214826237e-07 | 0 | 通过 |
| cad-part-159 source-fidelity | 7.705316429351683e-08 | 0 | 通过 |
| cad-part-160 source-fidelity | 1.17994817028233e-07 | 0 | 通过 |
| cad-part-161 source-fidelity | 1.1169462243998396e-07 | 0 | 通过 |
| cad-part-162 source-fidelity | 1.1760151474859981e-07 | 0 | 通过 |
| cad-part-163 source-fidelity | 1.0079593382172902e-07 | 0 | 通过 |
| cad-part-164 source-fidelity | 1.1502383667760796e-07 | 0 | 通过 |
| cad-part-165 source-fidelity | 1.1694721813171327e-07 | 0 | 通过 |
| cad-part-166 source-fidelity | 1.1793723864172989e-07 | 0 | 通过 |
| cad-part-167 source-fidelity | 1.1625717810659353e-07 | 0 | 通过 |
| cad-part-168 source-fidelity | 1.1905375707499388e-07 | 0 | 通过 |
| cad-part-169 source-fidelity | 1.1905683106050446e-07 | 0 | 通过 |
| cad-part-170 source-fidelity | 1.1723715553912939e-07 | 0 | 通过 |
| cad-part-171 source-fidelity | 1.1867782845342845e-07 | 0 | 通过 |
| cad-part-172 source-fidelity | 2.3597069098713064e-07 | 0 | 通过 |
| cad-part-173 source-fidelity | 2.3794281922562277e-07 | 0 | 通过 |
| cad-part-174 source-fidelity | 1.1907246255660198e-07 | 0 | 通过 |
| cad-part-175 source-fidelity | 2.3837835527729112e-07 | 0 | 通过 |
| cad-part-176 source-fidelity | 1.1883814066138143e-07 | 0 | 通过 |
| cad-part-177 source-fidelity | 2.3834907558750729e-07 | 0 | 通过 |
| cad-part-178 source-fidelity | 1.191702660996441e-07 | 0 | 通过 |
| cad-part-179 source-fidelity | 2.3824753725421033e-07 | 0 | 通过 |
| cad-part-180 source-fidelity | 1.1918328191029559e-07 | 0 | 通过 |
| cad-part-181 source-fidelity | 6.36343107096593e-08 | 0 | 通过 |
| cad-part-182 source-fidelity | 5.2150075813273133e-08 | 0 | 通过 |
| cad-part-183 source-fidelity | 1.1700391899793772e-07 | 0 | 通过 |
| cad-part-184 source-fidelity | 2.2633598106835962e-07 | 0 | 通过 |
| cad-part-185 source-fidelity | 1.7531596085973433e-07 | 0 | 通过 |
| cad-part-186 source-fidelity | 2.3817027727801587e-07 | 0 | 通过 |
| cad-part-187 source-fidelity | 2.3709277030548037e-07 | 0 | 通过 |
| cad-part-188 source-fidelity | 2.2560849544106532e-07 | 0 | 通过 |
| cad-part-189 source-fidelity | 2.3815195682175272e-07 | 0 | 通过 |
| cad-part-190 source-fidelity | 5.5550255062541964e-08 | 0 | 通过 |
| cad-part-191 source-fidelity | 5.38849733722202e-08 | 0 | 通过 |
| cad-part-192 source-fidelity | 5.929234103696501e-08 | 0 | 通过 |
| cad-part-193 source-fidelity | 5.5550255062541964e-08 | 0 | 通过 |
| cad-part-194 source-fidelity | 5.372503331102507e-08 | 0 | 通过 |
| cad-part-195 source-fidelity | 5.5309036905981657e-08 | 0 | 通过 |
| cad-part-196 source-fidelity | 1.191957350599182e-07 | 0 | 通过 |
| cad-part-197 source-fidelity | 1.0873716993486937e-07 | 0 | 通过 |
| cad-part-198 source-fidelity | 1.18123390180358e-07 | 0 | 通过 |
| cad-part-199 source-fidelity | 1.186017861698474e-07 | 0 | 通过 |
| cad-part-200 source-fidelity | 2.37652438173086e-07 | 0 | 通过 |
| cad-part-201 source-fidelity | 2.3534806548042297e-07 | 0 | 通过 |
| cad-part-202 source-fidelity | 2.3341413424304847e-07 | 0 | 通过 |
| cad-part-203 source-fidelity | 2.2825014323046844e-07 | 0 | 通过 |
| cad-part-204 source-fidelity | 1.1767155072561764e-07 | 0 | 通过 |
| cad-part-205 source-fidelity | 1.1908083408229686e-07 | 0 | 通过 |
| cad-part-206 source-fidelity | 1.1775509101141779e-07 | 0 | 通过 |
| cad-part-207 source-fidelity | 1.1902831786869683e-07 | 0 | 通过 |
| cad-part-208 source-fidelity | 2.3609654675738057e-07 | 0 | 通过 |
| cad-part-209 source-fidelity | 2.3048938047054435e-07 | 0 | 通过 |
| cad-part-210 source-fidelity | 2.3684673244872556e-07 | 0 | 通过 |
| cad-part-211 source-fidelity | 2.357742356906556e-07 | 0 | 通过 |
| cad-part-212 source-fidelity | 4.7654810231279043e-07 | 0 | 通过 |
| cad-part-213 source-fidelity | 3.48559572316276e-07 | 0 | 通过 |
| cad-part-214 source-fidelity | 4.5963146888539086e-07 | 0 | 通过 |
| cad-part-215 source-fidelity | 4.1867413891338856e-07 | 0 | 通过 |
| cad-part-216 source-fidelity | 4.7239655387443236e-07 | 0 | 通过 |
| cad-part-217 source-fidelity | 4.216711388949079e-07 | 0 | 通过 |
| cad-part-218 source-fidelity | 4.74093534208464e-07 | 0 | 通过 |
| cad-part-219 source-fidelity | 4.6093315475559393e-07 | 0 | 通过 |
| cad-part-220 source-fidelity | 3.927529395042484e-07 | 0 | 通过 |
| cad-part-221 source-fidelity | 4.7428955696204866e-07 | 0 | 通过 |
| cad-part-222 source-fidelity | 4.739683276966389e-07 | 0 | 通过 |
| cad-part-223 source-fidelity | 4.7446386908234217e-07 | 0 | 通过 |
| cad-part-224 source-fidelity | 3.927529395042484e-07 | 0 | 通过 |
| cad-part-225 source-fidelity | 4.699148785647367e-07 | 0 | 通过 |
| cad-part-226 source-fidelity | 3.6009918069623836e-07 | 0 | 通过 |
| cad-part-227 source-fidelity | 4.7624370225207713e-07 | 0 | 通过 |
| cad-part-228 source-fidelity | 1.1754152584586564e-07 | 0 | 通过 |
| cad-part-229 source-fidelity | 1.1900344176751787e-07 | 0 | 通过 |
| cad-part-230 source-fidelity | 1.1919527231896154e-07 | 0 | 通过 |
| cad-part-231 source-fidelity | 1.1777341102359173e-07 | 0 | 通过 |
| cad-part-232 source-fidelity | 1.192026055640838e-07 | 0 | 通过 |
| cad-part-233 source-fidelity | 1.1874576077985921e-07 | 0 | 通过 |
| cad-part-234 source-fidelity | 2.372112142268179e-07 | 0 | 通过 |
| cad-part-235 source-fidelity | 2.3493503675808824e-07 | 0 | 通过 |
| cad-part-236 source-fidelity | 2.3835738893751568e-07 | 0 | 通过 |
| cad-part-237 source-fidelity | 2.3721140163246446e-07 | 0 | 通过 |
| cad-part-238 source-fidelity | 2.349352241637348e-07 | 0 | 通过 |
| cad-part-239 source-fidelity | 2.3835757634316224e-07 | 0 | 通过 |
| cad-part-240 source-fidelity | 2.362643582998203e-07 | 0 | 通过 |
| cad-part-241 source-fidelity | 2.2146986999871388e-07 | 0 | 通过 |
| cad-part-242 source-fidelity | 2.383582016207697e-07 | 0 | 通过 |
| cad-part-243 source-fidelity | 1.1883311090699067e-07 | 0 | 通过 |
| cad-part-244 source-fidelity | 1.1884978290410686e-07 | 0 | 通过 |
| cad-part-245 source-fidelity | 1.1915806519269267e-07 | 0 | 通过 |
| cad-part-246 source-fidelity | 1.1754161954868891e-07 | 0 | 通过 |
| cad-part-247 source-fidelity | 1.1900334806469459e-07 | 0 | 通过 |
| cad-part-248 source-fidelity | 1.1919517861613826e-07 | 0 | 通过 |
| cad-part-249 source-fidelity | 2.211498264514944e-07 | 0 | 通过 |
| cad-part-250 source-fidelity | 2.3770230406228166e-07 | 0 | 通过 |
| cad-part-251 source-fidelity | 2.377177654722118e-07 | 0 | 通过 |
| cad-part-252 source-fidelity | 2.3322524533853084e-07 | 0 | 通过 |
| cad-part-253 source-fidelity | 2.381858061895059e-07 | 0 | 通过 |
| cad-part-254 source-fidelity | 2.3797836234962233e-07 | 0 | 通过 |
| cad-part-255 source-fidelity | 1.1911252206786571e-07 | 0 | 通过 |
| cad-part-256 source-fidelity | 1.189870140194671e-07 | 0 | 通过 |
| cad-part-257 source-fidelity | 1.1912166897332099e-07 | 0 | 通过 |
| cad-part-258 source-fidelity | 2.3770344004248045e-07 | 0 | 通过 |
| cad-part-259 source-fidelity | 2.3826937223248024e-07 | 0 | 通过 |
| cad-part-260 source-fidelity | 2.3840312568523814e-07 | 0 | 通过 |
| cad-part-261 source-fidelity | 2.2052859449672724e-07 | 0 | 通过 |
| cad-part-262 source-fidelity | 2.272545041037688e-07 | 0 | 通过 |
| cad-part-263 source-fidelity | 2.0201793393681555e-07 | 0 | 通过 |
| cad-part-264 source-fidelity | 2.3600703435988635e-07 | 0 | 通过 |
| cad-part-265 source-fidelity | 2.3744811983306136e-07 | 0 | 通过 |
| cad-part-266 source-fidelity | 2.3635574386560165e-07 | 0 | 通过 |
| cad-part-267 source-fidelity | 2.3694761885906246e-07 | 0 | 通过 |
| cad-part-268 source-fidelity | 2.3640446222827904e-07 | 0 | 通过 |
| cad-part-269 source-fidelity | 2.3777731072982533e-07 | 0 | 通过 |
| cad-part-270 source-fidelity | 2.3799217885311919e-07 | 0 | 通过 |
| cad-part-271 source-fidelity | 2.3560373563213943e-07 | 0 | 通过 |
| cad-part-272 source-fidelity | 2.381791102123998e-07 | 0 | 通过 |
| cad-part-273 source-fidelity | 2.3799217885311919e-07 | 0 | 通过 |
| cad-part-274 source-fidelity | 2.3560373563213943e-07 | 0 | 通过 |
| cad-part-275 source-fidelity | 2.381791102123998e-07 | 0 | 通过 |
| cad-part-276 source-fidelity | 2.3799217885311919e-07 | 0 | 通过 |
| cad-part-277 source-fidelity | 2.3560373563213943e-07 | 0 | 通过 |
| cad-part-278 source-fidelity | 2.381791102123998e-07 | 0 | 通过 |
| cad-part-279 source-fidelity | 2.3799217885311919e-07 | 0 | 通过 |
| cad-part-280 source-fidelity | 2.3560373563213943e-07 | 0 | 通过 |
| cad-part-281 source-fidelity | 2.381791102123998e-07 | 0 | 通过 |
| cad-part-282 source-fidelity | 2.3799270998381417e-07 | 0 | 通过 |
| cad-part-283 source-fidelity | 2.3560426676283441e-07 | 0 | 通过 |
| cad-part-284 source-fidelity | 2.3817964134309477e-07 | 0 | 通过 |
| cad-part-285 source-fidelity | 2.3819402272806656e-07 | 0 | 通过 |
| cad-part-286 source-fidelity | 2.3719935171584439e-07 | 0 | 通过 |
| cad-part-287 source-fidelity | 2.3746133770430333e-07 | 0 | 通过 |
| cad-part-288 source-fidelity | 2.379922747763885e-07 | 0 | 通过 |
| cad-part-289 source-fidelity | 2.3560382977905192e-07 | 0 | 通过 |
| cad-part-290 source-fidelity | 2.3817920435931228e-07 | 0 | 通过 |
| cad-part-291 source-fidelity | 1.1754152584586564e-07 | 0 | 通过 |
| cad-part-292 source-fidelity | 1.1900344176751787e-07 | 0 | 通过 |
| cad-part-293 source-fidelity | 1.1919527231896154e-07 | 0 | 通过 |
| cad-part-294 source-fidelity | 2.381385471039721e-07 | 0 | 通过 |
| cad-part-295 source-fidelity | 2.3826874695487277e-07 | 0 | 通过 |
| cad-part-296 source-fidelity | 2.384037509628456e-07 | 0 | 通过 |
| cad-part-297 source-fidelity | 2.3830284057169138e-07 | 0 | 通过 |
| cad-part-298 source-fidelity | 2.3830284057169138e-07 | 0 | 通过 |
| cad-part-299 source-fidelity | 2.3826533901427638e-07 | 0 | 通过 |
| cad-part-300 source-fidelity | 2.3597069098713064e-07 | 0 | 通过 |
| width | 17.600000381469727 | 17.6 | 通过 |
| height | 19.062532424926758 | 19.1 | 通过 |
| depth | 3.3609981536865234 | 3.4 | 通过 |

实际平台 `manufacturer-model.js` 的 headless 渲染与原照片对照在 [verification.json](../evidence/m5stack-s016/verification.json)：

- front: pass; IoU 0.984818822152397
- back: pass; IoU 0.9723672132944255
- unshielded-oblique: qualitative-only; IoU 0.18207153935633966
- side: qualitative-only; IoU 0.40888191095659976

正反视图只有照片确属近正投影且原始型号一致时才用0.95轮廓门槛。侧视/斜视保留为定性检查，不以轮廓掩盖标签、组件或装配差异。引用照片的纹理不是独立验证依据；未通过/缺失项不会改写成通过。

厂家 CAD 保留 MIT；官方照片及其裁切纹理版权归厂家，未验证独立开放图片许可，不套用项目 MIT。屏幕照片中的固件像素不作为模拟输出。runtime 为 unavailable，官方固件入口仅参考，未验证版本/hash/镜像不填猜测值。

重生成：

```sh
python tools/onboard-lane-01.py prepare --ids m5stack-s016
python tools/onboard-lane-01.py build --ids m5stack-s016
```

通过受跟踪的 headless 浏览器打开 `web/platform/review-lane-01.html?id=m5stack-s016`；按 model reviewViews 保存800×800正反侧画布到 `output/playwright/m5stack-s016-front.png` 等，再运行：

```sh
python tools/onboard-lane-01.py verify --ids m5stack-s016
python tools/onboard-lane-01.py publish --ids m5stack-s016
```

当前限制：

- Manufacturer populated mechanical STEP retained; CPU/board/peripheral runtime unavailable.
- No physical hardware measurements; colors are photograph-informed approximations, not calibrated materials.
- Manufacturer embedded STEP has no RF shield cover shown in current official shielded photo. Unshielded official photo is also retained; no shield guessed from a generic box.
- PCB silk text is absent; manufacturer STEP colors are visual CAD hints, not calibrated physical materials.
