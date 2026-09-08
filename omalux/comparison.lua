-- SPDX-License-Identifier: GPL-3.0-or-later
-- Development-only, one-way replication; each process owns its pixelpipe.
local dt = require "darktable"
local mailbox = assert(os.getenv("OMALUX_COMPARISON_MAILBOX"))
local function log(message)
  io.stderr:write("[omalux comparison] " .. message .. "\n")
  io.stderr:flush()
end

dt.control.dispatch(function()
  log("bridge ready")
  local applied, last_error
  while not dt.control.ending do
    local file = io.open(mailbox, "r")
    local command = file and file:read("*a")
    if file then file:close() end
    if command and command ~= applied and dt.gui.current_view() == dt.gui.views.darkroom then
      local ok, err = pcall(function()
        local body = command:match("^omalux%-controls%-v1 %d+\n(.*)$")
        assert(body, "unsupported controls message")
        local controls = {}
        for line in body:gmatch("[^\n]+") do
            local module, parameter, value = line:match("^([%w_]+) ([%w_]+) ([%d.eE+%-]+)$")
            value = tonumber(value)
            assert(module and value and value == value and math.abs(value) < math.huge, "invalid control")
            controls[#controls+1] = {module=module, parameter=parameter, value=value}
        end
        assert(#controls > 0, "empty controls message")
        local enabled = {}
        for _, c in ipairs(controls) do
          if not enabled[c.module] then
            local result = dt.gui.action("iop/" .. c.module, "enable", "on", 1)
            assert(result == result, "module unavailable: " .. c.module)
            enabled[c.module] = true
          end
          local path = "iop/" .. c.module .. "/" .. c.parameter
          local result = dt.gui.action(path, "value", "set", c.value)
          assert(result == result, "control unavailable: " .. path)
        end
      end)
      if ok then
        applied, last_error = command, nil
        log("applied " .. command:gsub("\n", "; "))
      elseif tostring(err) ~= last_error then
        last_error = tostring(err)
        log(last_error)
      end
    end
    dt.control.sleep(50)
  end
end)
