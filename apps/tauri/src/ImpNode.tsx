import { memo } from "react";
import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import type { InputValues } from "imp-core-types";
import classes from "./ImpNode.module.css";

export type ImpNodeData = {
  impType: string;
  inputValues: InputValues;
  inputPorts: string[];
  outputPorts: string[];
};

export type ImpFlowNode = Node<ImpNodeData, "imp">;

function handleTop(index: number, count: number): string {
  if (count <= 1) {
    return "50%";
  }
  return `${((index + 1) / (count + 1)) * 100}%`;
}

function ImpNodeComponent({ data }: NodeProps<ImpFlowNode>) {
  const rows = Math.max(data.inputPorts.length, data.outputPorts.length, 1);

  return (
    <div
      className={classes.node}
      style={{ minHeight: 36 + rows * 18 }}
    >
      <div className={classes.header} title={data.impType}>
        {data.impType || "(unknown)"}
      </div>
      <div className={classes.ports}>
        <div className={classes.inputs}>
          {data.inputPorts.map((portId) => (
            <div key={`in-${portId}`} className={classes.portRow}>
              {portId}
            </div>
          ))}
        </div>
        <div className={classes.outputs}>
          {data.outputPorts.map((portId) => (
            <div key={`out-${portId}`} className={classes.portRow}>
              {portId}
            </div>
          ))}
        </div>
      </div>
      {data.inputPorts.map((portId, index) => (
        <Handle
          key={`handle-in-${portId}`}
          className={classes.handle}
          type="target"
          position={Position.Left}
          id={portId}
          isConnectable={false}
          style={{ top: handleTop(index, data.inputPorts.length) }}
        />
      ))}
      {data.outputPorts.map((portId, index) => (
        <Handle
          key={`handle-out-${portId}`}
          className={classes.handle}
          type="source"
          position={Position.Right}
          id={portId}
          isConnectable={false}
          style={{ top: handleTop(index, data.outputPorts.length) }}
        />
      ))}
    </div>
  );
}

export const ImpNode = memo(ImpNodeComponent);
