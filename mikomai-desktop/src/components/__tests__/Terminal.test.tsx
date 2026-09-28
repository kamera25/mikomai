import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { Terminal } from "../Terminal";

describe("Terminal component", () => {
  it("renders standard content correctly", () => {
    render(<Terminal content="hello world" />);
    expect(screen.getByText("hello world")).toBeInTheDocument();
  });

  it("applies comment syntax highlighting for lines starting with ! or #", () => {
    const configContent = "! This is a comment\n# This is also a comment\n#\n#show interface status";
    const { container } = render(<Terminal content={configContent} />);
    
    const comments = container.querySelectorAll(".terminal-comment");
    expect(comments).toHaveLength(2);
    expect(comments[0].textContent).toBe("! This is a comment");
    expect(comments[1].textContent).toBe("# This is also a comment");
    
    // The single hash should render as plain text, not a comment
    const hashElements = screen.getAllByText("#");
    expect(hashElements.length).toBeGreaterThan(0);
    hashElements.forEach(el => {
      expect(el.className).not.toContain("terminal-comment");
    });

    // The line `#show interface status` should not be a comment, but have keyword highlight for `show` and `interface`
    const lines = container.querySelectorAll(".terminal-line");
    expect(lines[3].querySelector(".terminal-comment")).toBeNull();
    
    const keywords = container.querySelectorAll(".terminal-keyword");
    const keywordTexts = Array.from(keywords).map(el => el.textContent);
    expect(keywordTexts).toContain("show");
    expect(keywordTexts).toContain("interface");
  });

  it("highlights config keywords, interfaces, IPs, and numbers", () => {
    const configContent = "interface GigabitEthernet1/1\n ip address 192.168.1.1 255.255.255.0";
    const { container } = render(<Terminal content={configContent} />);

    // Check keyword GigabitEthernet (interface class)
    const interfaces = container.querySelectorAll(".terminal-interface");
    expect(interfaces).toHaveLength(1);
    expect(interfaces[0].textContent).toBe("GigabitEthernet1/1");

    // Check keyword
    const keywords = container.querySelectorAll(".terminal-keyword");
    expect(keywords).toHaveLength(2);
    expect(keywords[0].textContent).toBe("interface");
    expect(keywords[1].textContent).toBe("ip address");

    // Check IP address
    const ips = container.querySelectorAll(".terminal-ip");
    expect(ips).toHaveLength(2);
    expect(ips[0].textContent).toBe("192.168.1.1");
    expect(ips[1].textContent).toBe("255.255.255.0");
  });

  it("applies diff classes for lines starting with + or -", () => {
    const diffContent = "+ interface GigabitEthernet1/2\n- shutdown";
    const { container } = render(<Terminal content={diffContent} />);

    const adds = container.querySelectorAll(".terminal-diff-add");
    const removes = container.querySelectorAll(".terminal-diff-remove");

    expect(adds).toHaveLength(1);
    expect(removes).toHaveLength(1);

    expect(adds[0].querySelector(".terminal-interface")?.textContent).toBe("GigabitEthernet1/2");
    expect(removes[0].querySelector(".terminal-keyword")?.textContent).toBe("shutdown");
  });

  it("highlights MAC addresses as a single block in various formats", () => {
    const configContent = "mac 00:11:22:33:aa:bb\nmac 00-11-22-33-bb-cc\nmac 0011.2233.ccdd";
    const { container } = render(<Terminal content={configContent} />);

    const macs = container.querySelectorAll(".terminal-mac");
    expect(macs).toHaveLength(3);
    expect(macs[0].textContent).toBe("00:11:22:33:aa:bb");
    expect(macs[1].textContent).toBe("00-11-22-33-bb-cc");
    expect(macs[2].textContent).toBe("0011.2233.ccdd");
  });

  it("highlights UP/DOWN status words case-insensitively", () => {
    const configContent = "status UP\nstatus down\nstatus Up";
    const { container } = render(<Terminal content={configContent} />);

    const ups = container.querySelectorAll(".terminal-status-up");
    const downs = container.querySelectorAll(".terminal-status-down");

    expect(ups).toHaveLength(2);
    expect(downs).toHaveLength(1);
    expect(ups[0].textContent).toBe("UP");
    expect(ups[1].textContent).toBe("Up");
    expect(downs[0].textContent).toBe("down");
  });

  it("highlights double-quoted strings correctly", () => {
    const configContent = 'description "Core Switch Link"';
    const { container } = render(<Terminal content={configContent} />);

    const strings = container.querySelectorAll(".terminal-string");
    expect(strings).toHaveLength(1);
    expect(strings[0].textContent).toBe('"Core Switch Link"');
  });

  it("highlights FQDNs and IPv6 addresses correctly as a single block", () => {
    const configContent = "host core-switch-01.tokyo.office.net\nipv6 address 2001:db8::1/64\nipv6 address fe80::211:22ff:fe33:4455";
    const { container } = render(<Terminal content={configContent} />);

    const fqdns = container.querySelectorAll(".terminal-fqdn");
    expect(fqdns).toHaveLength(1);
    expect(fqdns[0].textContent).toBe("core-switch-01.tokyo.office.net");

    const ips = container.querySelectorAll(".terminal-ip");
    // Should capture the two IPv6 addresses (the test checks if they are marked as terminal-ip)
    expect(ips).toHaveLength(2);
    expect(ips[0].textContent).toBe("2001:db8::1/64");
    expect(ips[1].textContent).toBe("fe80::211:22ff:fe33:4455");
  });
});
